//! `phxclaw evoluir`: o ciclo de auto-evolucao (SP000015). Escolhe UM item do backlog,
//! implementa numa worktree pelo laco normal do agente, roda os portoes e, so com tudo verde,
//! faz nascer o ramo `evolucao/<id>` no repositorio do produto -- e PARA ali, esperando o Go.
//!
//! Decisao do dono (09/10/2026), que este modulo nao reabre: **propoe e espera o Go**. Nada
//! aqui faz merge, empurra ou mexe no ramo do produto; `aprovar` so MARCA e devolve o comando
//! para o humano rodar.
//!
//! Decisoes que valem saber antes de mexer:
//!
//! - **O alcance e conferido no diff, em codigo, e nao no prompt -- e e LISTA DE PERMISSAO.**
//!   Negado por padrao, como o portao do motor: o que nao esta em `permitidos` da politica E
//!   no teto embutido `PERMISSAO_MAXIMA` e recusado, com o caminho. A lista de proibicoes que
//!   havia antes era incompleta por natureza e se contornava por indirecao (`Cargo.toml` de
//!   crate, `build.rs`, `.cargo/config.toml`, `#[path]` no `lib.rs` trocando um modulo vetado
//!   sem toca-lo); a revisao de seguranca de 09/10/2026 (achado A2) mediu todos verdes. O
//!   minimo vive aqui: a politica so APERTA, e a que afrouxa e recusada inteira.
//! - **Dentro do permitido, o CONTEUDO do diff tambem passa pelo mesmo motor**
//!   (`conferir_alcance`): `#[path]`, `include*!`, `env!`, `extern crate`, `#[proc_macro]`,
//!   simbolo exportado, linha `mod` em `lib.rs`/`main.rs`/`mod.rs`, e no codigo que nao e
//!   teste qualquer caminho fora do `std` puro (`std::process`, `std::fs`, `crate::`...).
//! - **O ramo so nasce no fim.** O trabalho acontece num clone raso dentro da pasta de uma
//!   tarefa-mae (o modelo de `nuvem.rs`): a worktree (`git_worktree`, a ferramenta de sempre)
//!   e o ramo existem SO no clone. O produto recebe o ramo por `git bundle` depois do ultimo
//!   portao verde; recusado ou vermelho, o clone e apagado e o produto nem soube.
//! - **O hospedeiro nunca roda git no clone.** O agente escreveu ali: o `.git/config` dele
//!   pode declarar filtro ou textconv. Todo git no clone vai pelo `rodar_git` (sandbox, com a
//!   configuracao neutralizada), e o produto le so o arquivo do bundle.
//! - **Nada aprendido muda comportamento sem Go.** Cada desfecho vai para `desfechos.jsonl`
//!   como evidencia (PENDENTE ou INFRUTIFERO, nunca FRUTIFERO: nada promove sozinho), e a
//!   escolha do item NAO le esse arquivo -- le so o backlog, a politica e as propostas
//!   abertas.

use crate::ferramentas::config_de_subagente;
use crate::git::{GitTool, WorktreeTool, analisar_diff, diff_do_repo, rodar_git};
use crate::motor::{Agent, AgentConfig, CancelFlag, Observer};
use crate::tarefa::{Task, TaskStatus};
use chrono::{DateTime, Utc};
use phxclaw_agent_core::{Tool, ToolContext};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

/// A politica, relativa a pasta do projeto.
pub const POLITICA: &str = "config/evolucao-politica.json";
/// Relatorios, registros e desfechos, relativos a pasta do projeto (fora do git: `.phxclaw/`
/// e ignorado).
pub const PASTA: &str = ".phxclaw/evolucao";
pub const DESFECHOS: &str = "desfechos.jsonl";
/// Todo ramo que este modulo cria comeca aqui; o fetch confere antes de escrever a ref.
pub const PREFIXO_DO_RAMO: &str = "evolucao/";
pub const PORTOES_CONHECIDOS: &[&str] = &["fmt", "clippy", "test"];

/// O TETO do alcance, em codigo: a politica escolhe `permitidos` so DENTRO dele (decisao do
/// dono de 09/10/2026, «alcance: ferramentas e testes»). Testes de crate, e os modulos de
/// ferramenta PURA listados um a um -- sem E/S, sem processo, sem papel de seguranca. Na
/// duvida, fora: `notebook.rs`, `documentos.rs`, `sessoes.rs` leem disco; `esquema.rs` valida
/// argumento de ferramenta (e guarda); `perguntas.rs` e a espera da aprovacao das regras;
/// `canais/cripto.rs` confere assinatura. Sobrou a calculadora (`capability` «calc», sem efeito).
pub const PERMISSAO_MAXIMA: &[&str] = &[
    "crates/*/tests/**",
    "crates/phxclaw-agent/src/calculadora.rs",
];

/// Fora SEMPRE, mesmo dentro do teto e com a politica dizendo o contrario: os crates de
/// seguranca inteiros e os testes que guardam seguranca. Teste enfraquecido e guarda tirada
/// sem ninguem tocar no codigo guardado.
pub const NUNCA: &[&str] = &[
    "crates/phxclaw-sandbox/",
    "crates/phxclaw-secret-broker/",
    "crates/phxclaw-key-provider/",
    "crates/phxclaw-egress-broker/",
    "crates/phxclaw-self-evolving-intelligence/",
    "crates/phxclaw-skill-evolution/",
    "crates/phxclaw-agent/tests/evolucao.rs",
    "crates/phxclaw-agent/tests/guardas.rs",
    "crates/phxclaw-agent/tests/sandbox_agente.rs",
    "crates/phxclaw-agent/tests/rbac.rs",
    "crates/phxclaw-agent/tests/segredos.rs",
    "crates/phxclaw-agent/tests/segredos_shell.rs",
    "crates/phxclaw-agent/tests/licenca.rs",
    "crates/phxclaw-agent/tests/gonogo.rs",
    "crates/phxclaw-agent/tests/gonogo_cli.rs",
    "crates/phxclaw-agent/tests/credencial_ci.rs",
    "crates/phxclaw-agent/tests/teto_extensao.rs",
    POLITICA,
    "crates/phxclaw-agent/src/evolucao.rs",
];

/// Nomes que mudam como o cargo, o rustc ou as ferramentas rodam, em qualquer pasta: o
/// `build.rs` e o `runner` do `.cargo/config.toml` executam no hospedeiro depois do merge.
const NOMES_PROIBIDOS: &[&str] = &[
    "Cargo.toml",
    "Cargo.lock",
    "build.rs",
    "rustfmt.toml",
    "clippy.toml",
];

const PRAZO_DO_GIT: Duration = Duration::from_secs(300);

// ------------------------------------------------------------------ politica

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemVetado {
    pub item: String,
    pub motivo: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Politica {
    /// O backlog estruturado (`docs/absorcao/phxclaw.json`), relativo ao projeto.
    pub backlog: String,
    /// O que o ciclo PODE tocar; o resto e negado. Cada padrao tem de caber em
    /// `PERMISSAO_MAXIMA`. Sem `#[serde(default)]` de proposito: politica sem a lista nao
    /// carrega (falha fechado).
    pub permitidos: Vec<String>,
    /// Aperto a mais, dentro do permitido. Nunca afrouxa: `NUNCA` vale com ou sem ele.
    #[serde(default)]
    pub vetados: Vec<String>,
    #[serde(default)]
    pub itens_vetados: Vec<ItemVetado>,
    pub portoes: Vec<String>,
    pub revisao_bloqueia_em: String,
}

impl Politica {
    /// Le e valida `<projeto>/config/evolucao-politica.json`. Ausente ou invalida e ERRO: sem
    /// politica nao ha alcance, e sem alcance o ciclo nao roda (falha fechado).
    pub fn carregar(projeto: &Path) -> Result<Self, String> {
        let p = projeto.join(POLITICA);
        let texto = std::fs::read_to_string(&p).map_err(|e| format!("{}: {e}", p.display()))?;
        let pol: Politica =
            serde_json::from_str(&texto).map_err(|e| format!("{}: {e}", p.display()))?;
        pol.validar().map_err(|e| format!("{}: {e}", p.display()))
    }

    pub fn validar(self) -> Result<Self, String> {
        let invalido = |v: &String| {
            v.trim().is_empty() || v.starts_with('/') || v.split('/').any(|s| s == "..")
        };
        if let Some(v) = self.vetados.iter().find(|v| invalido(v)) {
            return Err(format!("caminho vetado invalido: {v:?}"));
        }
        if self.permitidos.is_empty() {
            return Err("sem permitidos: o ciclo nao teria onde escrever".into());
        }
        // A politica so aperta: padrao que nao cabe no teto do codigo recusa a politica
        // INTEIRA, e nao so o padrao -- quem afrouxou a cerca nao escolhe o resto dela.
        for v in &self.permitidos {
            if invalido(v) {
                return Err(format!("caminho permitido invalido: {v:?}"));
            }
            if !PERMISSAO_MAXIMA.iter().any(|m| cabe(v, m)) {
                return Err(format!(
                    "permitido {v:?} e mais frouxo que o minimo do codigo ({}): a politica so \
pode apertar",
                    PERMISSAO_MAXIMA.join(", ")
                ));
            }
        }
        if self.portoes.is_empty() {
            return Err("sem portoes: item sem portao verde nao vira ramo".into());
        }
        if let Some(p) = self
            .portoes
            .iter()
            .find(|p| !PORTOES_CONHECIDOS.contains(&p.as_str()))
        {
            return Err(format!(
                "portao desconhecido: {p} (use {})",
                PORTOES_CONHECIDOS.join(", ")
            ));
        }
        if !crate::revisao::SEVERIDADES.contains(&self.revisao_bloqueia_em.as_str()) {
            return Err(format!(
                "revisao_bloqueia_em {:?}: use {}",
                self.revisao_bloqueia_em,
                crate::revisao::SEVERIDADES.join(", ")
            ));
        }
        Ok(self)
    }

    /// O padrao vetado que casa o caminho (relativo ao projeto), se algum.
    pub fn veto(&self, caminho: &str) -> Option<&str> {
        self.vetados
            .iter()
            .find(|p| casa(p, caminho))
            .map(String::as_str)
    }

    /// Permitido = casa um `permitidos` da politica E um padrao do teto do codigo. As duas
    /// conferencias, e nao so a da politica: a validacao ja recusa a frouxa, e esta e a
    /// segunda trava para o dia em que alguem montar uma `Politica` sem `validar`.
    pub fn permite(&self, caminho: &str) -> bool {
        self.permitidos.iter().any(|p| permite(p, caminho))
            && PERMISSAO_MAXIMA.iter().any(|p| permite(p, caminho))
    }

    pub fn item_vetado(&self, item: &str) -> Option<&ItemVetado> {
        self.itens_vetados.iter().find(|i| i.item == item)
    }
}

/// `dir/` veta a pasta inteira; `*` casa um trecho sem `/`; o resto e o arquivo exato ou a
/// pasta com esse nome.
pub fn casa(padrao: &str, caminho: &str) -> bool {
    if padrao.ends_with('/') {
        return caminho.starts_with(padrao);
    }
    if padrao.contains('*') {
        return glob(padrao.as_bytes(), caminho.as_bytes());
    }
    caminho == padrao
        || caminho
            .strip_prefix(padrao)
            .is_some_and(|r| r.starts_with('/'))
}

/// O casador da PERMISSAO, mais estreito que o do veto: literal e so o arquivo exato (nunca
/// a pasta com esse nome), `*` e um trecho sem `/`, e `/**` no fim e tudo abaixo da pasta.
pub fn permite(padrao: &str, caminho: &str) -> bool {
    if let Some(base) = padrao.strip_suffix("/**") {
        let n = base.split('/').count();
        let seg: Vec<&str> = caminho.split('/').collect();
        return seg.len() > n
            && seg[n..].iter().all(|s| !s.is_empty())
            && glob(base.as_bytes(), seg[..n].join("/").as_bytes());
    }
    if padrao.contains("**") {
        return false;
    }
    glob(padrao.as_bytes(), caminho.as_bytes())
}

/// O padrao `p` da politica cabe no padrao `m` do teto: tudo o que `p` permite, `m` permite.
/// Por trecho: `**` final de `m` cobre o resto (ao menos um trecho), `*` de `m` cobre um
/// trecho qualquer sem `**`, literal so cobre o mesmo literal. Conservador de proposito:
/// recusa padrao que talvez coubesse, nunca aceita um que nao cabe.
pub fn cabe(p: &str, m: &str) -> bool {
    let ps: Vec<&str> = p.split('/').collect();
    let ms: Vec<&str> = m.split('/').collect();
    for (i, mt) in ms.iter().enumerate() {
        if *mt == "**" {
            return i == ms.len() - 1 && ps.len() > i && ps[..i].len() == i;
        }
        let Some(pt) = ps.get(i) else { return false };
        let ok = if *mt == "*" {
            !pt.is_empty() && !pt.contains("**")
        } else {
            !mt.contains('*') && pt == mt
        };
        if !ok {
            return false;
        }
    }
    ps.len() == ms.len()
}

fn glob(p: &[u8], s: &[u8]) -> bool {
    match p.split_first() {
        None => s.is_empty(),
        Some((b'*', resto)) => {
            // `*` come de 0 ate o proximo `/` (exclusive): nunca atravessa pasta.
            let mut i = 0;
            loop {
                if glob(resto, &s[i..]) {
                    return true;
                }
                if i == s.len() || s[i] == b'/' {
                    return false;
                }
                i += 1;
            }
        }
        Some((c, resto)) => s.first() == Some(c) && glob(resto, &s[1..]),
    }
}

// ------------------------------------------------------------------ portao do alcance

/// Uma entrada do `git diff --raw -z --no-renames`.
#[derive(Debug, Clone, PartialEq)]
pub struct Mudanca {
    pub caminho: String,
    pub modo_novo: String,
    pub estado: char,
}

pub fn analisar_raw(saida: &str) -> Vec<Mudanca> {
    let mut v = Vec::new();
    let mut partes = saida.split('\0');
    while let Some(meta) = partes.next() {
        let Some(meta) = meta.trim_start_matches('\n').strip_prefix(':') else {
            continue;
        };
        let Some(caminho) = partes.next() else { break };
        let campos: Vec<&str> = meta.split_whitespace().collect();
        v.push(Mudanca {
            caminho: caminho.to_string(),
            modo_novo: campos.get(1).copied().unwrap_or("").to_string(),
            estado: campos.get(4).and_then(|s| s.chars().next()).unwrap_or('?'),
        });
    }
    v
}

/// Uma secao do diff unificado (`-U0 --text --no-renames`): o que entrou e o que saiu.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Secao {
    pub caminho: String,
    pub adicionadas: Vec<String>,
    pub removidas: Vec<String>,
    pub binario: bool,
}

/// Le o diff unificado. Cabecalho que nao e `a/X b/X` vira secao com o caminho cru (e
/// nenhum caminho do `--raw` casa com ela: recusa por falta de conteudo, nunca aceite).
pub fn analisar_conteudo(texto: &str) -> Vec<Secao> {
    let mut v: Vec<Secao> = Vec::new();
    let (mut falta_a, mut falta_b) = (0u64, 0u64);
    for l in texto.lines() {
        if (falta_a > 0 || falta_b > 0)
            && let Some(s) = v.last_mut()
        {
            match l.as_bytes().first() {
                Some(b'+') => {
                    s.adicionadas.push(l[1..].to_string());
                    falta_b = falta_b.saturating_sub(1);
                }
                Some(b'-') => {
                    s.removidas.push(l[1..].to_string());
                    falta_a = falta_a.saturating_sub(1);
                }
                Some(b'\\') => {}
                _ => {
                    falta_a = falta_a.saturating_sub(1);
                    falta_b = falta_b.saturating_sub(1);
                }
            }
            continue;
        }
        if let Some(r) = l.strip_prefix("diff --git ") {
            let caminho = r
                .strip_prefix("a/")
                .and_then(|x| x.split_once(" b/"))
                .filter(|(a, b)| a == b)
                .map(|(a, _)| a.to_string())
                .unwrap_or_else(|| r.to_string());
            v.push(Secao {
                caminho,
                ..Secao::default()
            });
        } else if l.starts_with("Binary files ") || l.starts_with("GIT binary patch") {
            if let Some(s) = v.last_mut() {
                s.binario = true;
            }
        } else if let Some(h) = l.strip_prefix("@@ ") {
            let mut it = h.split_whitespace();
            let conta = |x: Option<&str>, sinal: char| -> u64 {
                x.and_then(|x| x.strip_prefix(sinal))
                    .map(|x| x.split_once(',').map(|(_, n)| n).unwrap_or("1"))
                    .and_then(|n| n.parse().ok())
                    .unwrap_or(0)
            };
            falta_a = conta(it.next(), '-');
            falta_b = conta(it.next(), '+');
        }
    }
    v
}

/// So o alfabeto que dispensa aspas: caminho com espaco, aspas ou controle mudaria a forma do
/// cabecalho do diff unificado, e o conteudo se leria contra o arquivo errado.
fn caminho_seguro(c: &str) -> bool {
    !c.is_empty()
        && c.chars()
            .all(|x| x.is_ascii_alphanumeric() || "._/-".contains(x))
}

/// `crates/<crate>/tests/...`: onde a evolucao escreve teste.
fn e_teste(rel: &str) -> bool {
    let s: Vec<&str> = rel.split('/').collect();
    s.len() >= 4 && s[0] == "crates" && s[2] == "tests"
}

/// O motivo de recusa do CAMINHO, se algum. A ordem importa so para a mensagem: o primeiro
/// motivo e o que se mostra.
fn recusa_do_caminho(pol: &Politica, rel: &str, m: &Mudanca) -> Option<String> {
    if !caminho_seguro(rel) {
        return Some("caminho com caractere fora de [A-Za-z0-9._/-]".into());
    }
    if let Some(p) = NUNCA.iter().find(|p| casa(p, rel)) {
        return Some(format!("vetado pelo minimo do codigo ({p})"));
    }
    if let Some(p) = pol.veto(rel) {
        return Some(format!("caminho vetado ({p})"));
    }
    if !pol.permite(rel) {
        return Some(format!(
            "fora da lista de permissao (alcance: ferramentas e testes; permitidos: {})",
            pol.permitidos.join(", ")
        ));
    }
    let nome = rel.rsplit('/').next().unwrap_or(rel);
    if rel.split('/').any(|s| s.starts_with('.')) {
        return Some("arquivo ou pasta oculta (.cargo/, .git*, .envrc...)".into());
    }
    if NOMES_PROIBIDOS.contains(&nome) || nome.starts_with("rust-toolchain") {
        return Some(format!("{nome} muda como o cargo roda: exige humano"));
    }
    if !nome.ends_with(".rs") && !e_teste(rel) {
        return Some("arquivo que nao e .rs fora de testes".into());
    }
    // Link e submodulo: o caminho que o diff mostra nao e o que eles alcancam.
    match (m.estado, m.modo_novo.as_str()) {
        ('D', _) => Some("apagar arquivo exige humano".into()),
        (_, "120000") => Some("link simbolico".into()),
        (_, "160000") => Some("submodulo".into()),
        (_, "100755") => Some("modo executavel".into()),
        (_, "100644") => None,
        (_, outro) => Some(format!("modo de arquivo {outro:?}")),
    }
}

/// As fichas de um trecho de Rust: identificador inteiro, `::` junto, e o resto um caractere
/// por ficha. Comentario e texto NAO saem: com `-U0` nao se sabe se a linha esta dentro de um
/// comentario ou de uma string aberta antes, e um analisador que errasse esse estado pularia
/// codigo de verdade. Falso positivo recusa (e o humano decide); falso negativo deixaria passar.
fn fichas(t: &str) -> Vec<String> {
    let c: Vec<char> = t.chars().collect();
    let mut v = Vec::new();
    let mut i = 0;
    while i < c.len() {
        let x = c[i];
        if x.is_alphanumeric() || x == '_' {
            let ini = i;
            while i < c.len() && (c[i].is_alphanumeric() || c[i] == '_') {
                i += 1;
            }
            v.push(c[ini..i].iter().collect());
            continue;
        }
        if x == ':' && c.get(i + 1) == Some(&':') {
            v.push("::".into());
            i += 2;
            continue;
        }
        if !x.is_whitespace() {
            v.push(x.to_string());
        }
        i += 1;
    }
    v
}

/// Em qualquer .rs (teste ou nao): o que muda o que o compilador LE ou LIGA sem que o caminho
/// do arquivo diga. `include` sozinho cobre o identificador passado a uma macro.
const PALAVRAS_SEMPRE: &[(&str, &str)] = &[
    ("include", "include!"),
    ("include_str", "include_str!"),
    ("include_bytes", "include_bytes!"),
    ("option_env", "option_env!"),
    ("cfg_attr", "cfg_attr (pode aplicar #[path])"),
    ("macro_rules", "macro_rules! (esconde nome de macro)"),
    ("proc_macro", "#[proc_macro]"),
    ("proc_macro_derive", "#[proc_macro_derive]"),
    ("proc_macro_attribute", "#[proc_macro_attribute]"),
    ("no_mangle", "simbolo exportado (no_mangle)"),
    ("export_name", "simbolo exportado (export_name)"),
    ("link_section", "secao de ligacao (link_section)"),
    ("link_name", "simbolo ligado (link_name)"),
    ("global_allocator", "#[global_allocator]"),
    ("global_asm", "global_asm!"),
    ("naked", "funcao naked"),
];

/// So no codigo que NAO e teste: o teste roda no sandbox dos portoes e pode chamar processo e
/// ler disco; a ferramenta roda no processo do agente, sem sandbox nenhum.
const PALAVRAS_FORA_DE_TESTE: &[(&str, &str)] = &[
    ("unsafe", "unsafe"),
    ("extern", "extern"),
    ("Command", "std::process::Command"),
    ("process", "std::process"),
    ("spawn", "processo ou thread novo"),
    ("asm", "asm!"),
    ("read_dir", "E/S de arquivo (read_dir)"),
    ("read_link", "E/S de arquivo (read_link)"),
    ("metadata", "E/S de arquivo (metadata)"),
    ("symlink_metadata", "E/S de arquivo (symlink_metadata)"),
    ("canonicalize", "E/S de arquivo (canonicalize)"),
    ("exists", "E/S de arquivo (exists)"),
    ("try_exists", "E/S de arquivo (try_exists)"),
    ("is_file", "E/S de arquivo (is_file)"),
    ("is_dir", "E/S de arquivo (is_dir)"),
    ("is_symlink", "E/S de arquivo (is_symlink)"),
];

/// A raiz de caminho que o codigo que nao e teste pode usar: primitivo, `serde_json`, o
/// proprio modulo, e o `std` nos modulos sem E/S. `crate::`, `super::` e outro crate ficam
/// fora: por eles uma ferramenta pura alcancaria o `git.rs` ou o `sistema.rs` sem toca-los.
const RAIZES_PURAS: &[&str] = &[
    "self",
    "serde_json",
    "f32",
    "f64",
    "i8",
    "i16",
    "i32",
    "i64",
    "i128",
    "isize",
    "u8",
    "u16",
    "u32",
    "u64",
    "u128",
    "usize",
    "char",
    "str",
    "bool",
];
const STD_PURO: &[&str] = &[
    "fmt",
    "collections",
    "cmp",
    "iter",
    "str",
    "string",
    "num",
    "ops",
    "convert",
    "borrow",
    "char",
    "f32",
    "f64",
    "i32",
    "i64",
    "u8",
    "u32",
    "u64",
    "usize",
    "mem",
    "slice",
    "vec",
    "option",
    "result",
    "hash",
    "marker",
    "array",
    "ascii",
    "default",
    "boxed",
];

/// Os motivos de recusa do CONTEUDO de um .rs ja permitido pelo caminho.
fn recusas_do_conteudo(rel: &str, s: &Secao) -> Vec<String> {
    let mut r: Vec<String> = Vec::new();
    let mut por = |m: String| {
        if !r.contains(&m) {
            r.push(m);
        }
    };
    if s.binario {
        por("conteudo binario: o que nao se le nao se aprova".into());
        return r;
    }
    let teste = e_teste(rel);
    let nome = rel.rsplit('/').next().unwrap_or(rel);
    let f = fichas(&s.adicionadas.join("\n"));
    let fs: Vec<&str> = f.iter().map(String::as_str).collect();
    for (i, t) in fs.iter().enumerate() {
        let prox = fs.get(i + 1).copied();
        if let Some((_, m)) = PALAVRAS_SEMPRE.iter().find(|(p, _)| p == t) {
            por(m.to_string());
        }
        if !teste && let Some((_, m)) = PALAVRAS_FORA_DE_TESTE.iter().find(|(p, _)| p == t) {
            por(m.to_string());
        }
        match (*t, prox) {
            ("env", Some("!")) => por("env!".into()),
            ("extern", Some("crate")) => por("extern crate".into()),
            ("#", _) => {
                let mut j = if prox == Some("!") { i + 2 } else { i + 1 };
                if fs.get(j) == Some(&"[") {
                    j += 1;
                    // `r#path`: o identificador cru e o mesmo atributo.
                    if fs.get(j) == Some(&"r") && fs.get(j + 1) == Some(&"#") {
                        j += 2;
                    }
                    if let Some(a @ (&"path" | &"link")) = fs.get(j) {
                        por(format!("atributo #[{a}]"));
                    }
                }
            }
            // `mod x;` (ate o primeiro `;` ou `{`): carrega ARQUIVO. O `mod x { }` em linha
            // nao carrega nada e passa.
            ("mod", _) if !teste => {
                let fim = fs[i + 1..].iter().find(|t| **t == ";" || **t == "{");
                if fim == Some(&";") {
                    por("mod declarado fora de teste: criar modulo exige humano".into());
                }
            }
            ("::", _) if !teste => {
                if let Some(m) = raiz_impura(&fs, i) {
                    por(m);
                }
            }
            _ => {}
        }
    }
    // Em lib.rs/main.rs/mod.rs, a linha `mod` que entra OU sai muda o que compila.
    if matches!(nome, "lib.rs" | "main.rs" | "mod.rs") {
        let toque = format!("{}\n{}", s.adicionadas.join("\n"), s.removidas.join("\n"));
        if fichas(&toque).iter().any(|t| t == "mod") {
            por(format!(
                "linha mod em {nome}: criar ou trocar modulo exige humano"
            ));
        }
    }
    r
}

/// Confere a raiz do caminho que tem `::` na posicao `i`. `None` = permitido.
fn raiz_impura(fs: &[&str], i: usize) -> Option<String> {
    let prox = fs.get(i + 1).copied();
    if prox == Some("<") {
        return None; // turbofish: `collect::<Vec<_>>`
    }
    let Some(ant) = i.checked_sub(1).map(|j| fs[j]) else {
        return Some("caminho absoluto (::)".into());
    };
    if ant == ">" {
        return None; // `<T as Tr>::f`
    }
    if !ant.chars().all(|c| c.is_alphanumeric() || c == '_') {
        return Some("caminho absoluto (::)".into());
    }
    if i >= 2 && fs[i - 2] == "::" {
        return None; // trecho do meio: a raiz ja foi conferida
    }
    if ant.chars().next().is_some_and(char::is_uppercase) {
        return None; // tipo, enum ou trait em escopo
    }
    if RAIZES_PURAS.contains(&ant) {
        return None;
    }
    if matches!(ant, "std" | "core" | "alloc") {
        return match prox {
            Some(m) if STD_PURO.contains(&m) => None,
            Some(m) => Some(format!("{ant}::{m} fora do std puro")),
            None => Some(format!("{ant}:: sem modulo")),
        };
    }
    Some(format!("caminho {ant}:: fora do std puro"))
}

/// O motor UNICO do alcance: os motivos de recusa do diff; vazio = dentro do alcance.
/// `mudancas` e o `--raw -z --no-renames`; `conteudo` e o diff unificado do mesmo par de
/// commits (`-U0 --text --no-renames`). `prefixo` e a pasta do projeto dentro do
/// repositorio (`phxclaw/`), como o `git rev-parse --show-prefix` a da.
pub fn conferir_alcance(
    pol: &Politica,
    prefixo: &str,
    mudancas: &[Mudanca],
    conteudo: &str,
) -> Vec<String> {
    let secoes = analisar_conteudo(conteudo);
    let mut recusas = Vec::new();
    for m in mudancas {
        let Some(rel) = m.caminho.strip_prefix(prefixo) else {
            recusas.push(format!("{}: fora do projeto ({prefixo})", m.caminho));
            continue;
        };
        if let Some(motivo) = recusa_do_caminho(pol, rel, m) {
            recusas.push(format!("{rel}: {motivo}"));
            continue;
        }
        let Some(s) = secoes.iter().find(|s| s.caminho == m.caminho) else {
            recusas.push(format!(
                "{rel}: sem conteudo no diff (nao se aprova o que nao se leu)"
            ));
            continue;
        };
        if rel.ends_with(".rs") || s.binario {
            for motivo in recusas_do_conteudo(rel, s) {
                recusas.push(format!("{rel}: conteudo recusado: {motivo}"));
            }
        }
    }
    // Secao no conteudo sem entrada no --raw: os dois diffs nao contam a mesma historia.
    for s in &secoes {
        if !mudancas.iter().any(|m| m.caminho == s.caminho) {
            recusas.push(format!(
                "{}: no diff unificado e fora do --raw (cabecalho que nao se le)",
                s.caminho
            ));
        }
    }
    recusas
}

// ------------------------------------------------------------------ backlog e escolha

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Item {
    pub nome: String,
    pub estado: String,
    pub evidencia: String,
}

pub fn ler_backlog(projeto: &Path, pol: &Politica) -> Result<Vec<Item>, String> {
    let p = projeto.join(&pol.backlog);
    let texto = std::fs::read_to_string(&p).map_err(|e| format!("{}: {e}", p.display()))?;
    let v: Value = serde_json::from_str(&texto).map_err(|e| format!("{}: {e}", p.display()))?;
    let estados = v
        .get("estados")
        .and_then(Value::as_object)
        .ok_or_else(|| format!("{}: sem o objeto 'estados'", p.display()))?;
    Ok(estados
        .iter()
        .map(|(nome, x)| Item {
            nome: nome.clone(),
            estado: x["estado"].as_str().unwrap_or("").to_string(),
            evidencia: x["evidencia"].as_str().unwrap_or("").to_string(),
        })
        .collect())
}

/// Os candidatos, na ordem da escolha: `parcial` antes de `nao` (o passo menor, sobre o que
/// ja existe), depois o nome. Fora: item vetado pela politica e item com proposta esperando Go.
pub fn candidatos(projeto: &Path, pol: &Politica) -> Result<Vec<Item>, String> {
    let abertos = itens_abertos(projeto)?;
    let mut v: Vec<Item> = ler_backlog(projeto, pol)?
        .into_iter()
        .filter(|i| matches!(i.estado.as_str(), "parcial" | "nao"))
        .filter(|i| pol.item_vetado(&i.nome).is_none() && !abertos.contains(&i.nome))
        .collect();
    v.sort_by(|a, b| (a.estado != "parcial", &a.nome).cmp(&(b.estado != "parcial", &b.nome)));
    Ok(v)
}

/// O item do ciclo e o porque. `pedido` e o `--item` do operador.
pub fn escolher(
    projeto: &Path,
    pol: &Politica,
    pedido: Option<&str>,
) -> Result<(Item, String), String> {
    if let Some(nome) = pedido {
        let item = ler_backlog(projeto, pol)?
            .into_iter()
            .find(|i| i.nome == nome)
            .ok_or_else(|| format!("item {nome:?} nao esta em {}", pol.backlog))?;
        if let Some(v) = pol.item_vetado(nome) {
            return Err(format!(
                "item {nome:?} fora do alcance: {} ({POLITICA})",
                v.motivo
            ));
        }
        if itens_abertos(projeto)?.contains(&item.nome) {
            return Err(format!(
                "item {nome:?} ja tem proposta esperando Go (`evoluir listar`)"
            ));
        }
        return Ok((item, "pedido pelo operador (--item)".into()));
    }
    let todos = ler_backlog(projeto, pol)?;
    let pendentes = todos
        .iter()
        .filter(|i| matches!(i.estado.as_str(), "parcial" | "nao"))
        .count();
    let lista = candidatos(projeto, pol)?;
    let Some(item) = lista.first().cloned() else {
        return Err(format!(
            "nenhum candidato: {pendentes} itens parcial/nao no backlog, todos vetados pela politica \
ou com proposta esperando Go"
        ));
    };
    let porque = format!(
        "primeiro de {} candidatos: estado «{}» ({}), fora dos {} itens vetados pela politica; \
{} itens parcial/nao no backlog",
        lista.len(),
        item.estado,
        if item.estado == "parcial" {
            "parcial vem antes de nao: o passo menor, sobre o que ja existe"
        } else {
            "nao ha parcial no alcance"
        },
        pol.itens_vetados.len(),
        pendentes
    );
    Ok((item, porque))
}

fn itens_abertos(projeto: &Path) -> Result<Vec<String>, String> {
    Ok(listar(projeto)?
        .into_iter()
        .filter(|r| r.estado == Estado::EsperandoGo)
        .map(|r| r.item)
        .collect())
}

// ------------------------------------------------------------------ registro

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Estado {
    EsperandoGo,
    Aprovado,
    Rejeitado,
    /// Algum portao (plano, laco, compilacao, teste, revisao) nao passou: o ramo nao nasceu.
    Vermelho,
    /// O diff tocou caminho vetado: o ramo nao nasceu.
    Recusado,
}

impl Estado {
    pub fn nome(self) -> &'static str {
        match self {
            Estado::EsperandoGo => "esperando Go",
            Estado::Aprovado => "aprovado (merge e do humano)",
            Estado::Rejeitado => "rejeitado pelo humano",
            Estado::Vermelho => "vermelho",
            Estado::Recusado => "recusado pelo portao do alcance",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Portao {
    pub nome: String,
    pub alvo: String,
    pub verde: bool,
    pub segundos: f64,
    pub detalhe: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ArquivoMudado {
    pub caminho: String,
    pub estado: String,
    pub adicoes: u64,
    pub remocoes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Registro {
    pub id: String,
    pub item: String,
    pub porque: String,
    pub criado_em: DateTime<Utc>,
    /// O `id()` do modelo que rodou de verdade, nunca o pedido.
    pub modelo: String,
    #[serde(default)]
    pub nota_do_modelo: Option<String>,
    pub repositorio: String,
    pub base: String,
    pub ramo: String,
    /// O commit que os portoes conferiram; `aprovar` recusa se o ramo nao apontar mais para ele.
    #[serde(default)]
    pub commit: Option<String>,
    pub estado: Estado,
    /// Onde parou (plano, laco, commit, alcance, portao:NOME, revisao, exportar), quando parou.
    #[serde(default)]
    pub etapa: Option<String>,
    #[serde(default)]
    pub motivo: Option<String>,
    #[serde(default)]
    pub plano: Vec<String>,
    #[serde(default)]
    pub tarefa: Option<String>,
    #[serde(default)]
    pub arquivos: Vec<ArquivoMudado>,
    #[serde(default)]
    pub portoes: Vec<Portao>,
    #[serde(default)]
    pub revisao: Option<Value>,
    #[serde(default)]
    pub decidido_em: Option<DateTime<Utc>>,
}

fn pasta(projeto: &Path) -> PathBuf {
    projeto.join(PASTA)
}

/// Id vindo da CLI vira nome de arquivo: so o alfabeto dos ids que este modulo gera.
fn validar_id(id: &str) -> Result<(), String> {
    if id.is_empty()
        || id.starts_with(['.', '-'])
        || !id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "._-".contains(c))
    {
        return Err(format!("id de evolucao invalido: {id:?}"));
    }
    Ok(())
}

pub fn carregar(projeto: &Path, id: &str) -> Result<Registro, String> {
    validar_id(id)?;
    let p = pasta(projeto).join(format!("{id}.json"));
    let b = std::fs::read(&p).map_err(|e| format!("{}: {e}", p.display()))?;
    serde_json::from_slice(&b).map_err(|e| format!("{}: {e}", p.display()))
}

/// Todos os registros, mais novos primeiro.
pub fn listar(projeto: &Path) -> Result<Vec<Registro>, String> {
    let dir = pasta(projeto);
    let entradas = match std::fs::read_dir(&dir) {
        Ok(e) => e,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(vec![]),
        Err(e) => return Err(format!("{}: {e}", dir.display())),
    };
    let mut v = Vec::new();
    for e in entradas.flatten() {
        let p = e.path();
        if p.extension().is_some_and(|x| x == "json") {
            let b = std::fs::read(&p).map_err(|e| format!("{}: {e}", p.display()))?;
            v.push(
                serde_json::from_slice::<Registro>(&b)
                    .map_err(|e| format!("{}: {e}", p.display()))?,
            );
        }
    }
    v.sort_by_key(|r| std::cmp::Reverse(r.criado_em));
    Ok(v)
}

fn gravar(projeto: &Path, r: &Registro) -> Result<(), String> {
    let dir = pasta(projeto);
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let j = serde_json::to_vec_pretty(r).map_err(|e| e.to_string())?;
    phxclaw_types::arquivo::gravar_atomico(&dir.join(format!("{}.json", r.id)), &j)
        .map_err(|e| e.to_string())?;
    phxclaw_types::arquivo::gravar_atomico(
        &dir.join(format!("{}.md", r.id)),
        relatorio(r).as_bytes(),
    )
    .map_err(|e| e.to_string())
}

/// A evidencia de um desfecho. `aprendizado` nunca e FRUTIFERO aqui: promover pede evidencia
/// validada por alguem, e nada promove sozinho (pétrea de 24/09/2026).
fn anotar_desfecho(projeto: &Path, r: &Registro, desfecho: &str) -> Result<(), String> {
    use std::io::Write;
    let infrutifero = matches!(
        r.estado,
        Estado::Vermelho | Estado::Recusado | Estado::Rejeitado
    );
    let prevencao = infrutifero.then(|| prevencao(r));
    let linha = json!({
        "id": r.id,
        "item": r.item,
        "desfecho": desfecho,
        "quando": Utc::now(),
        "commit": r.commit,
        "modelo": r.modelo,
        "etapa": r.etapa,
        "aprendizado": if infrutifero { "INFRUTIFERO" } else { "PENDENTE" },
        "causa": if infrutifero { r.motivo.clone() } else { None },
        "prevencao": prevencao,
    });
    let dir = pasta(projeto);
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join(DESFECHOS))
        .map_err(|e| e.to_string())?;
    writeln!(f, "{linha}").map_err(|e| e.to_string())
}

fn prevencao(r: &Registro) -> String {
    match r.etapa.as_deref().unwrap_or("") {
        "alcance" => "o item so avancou fora da lista de permissao ou com conteudo recusado: \
so o humano decide se ele sai do alcance (itens_vetados)"
            .into(),
        e if e.starts_with("portao:") => format!(
            "rodar rust_project {} dentro do laco antes de entregar",
            e.trim_start_matches("portao:")
        ),
        "revisao" => "corrigir o achado da revisao antes do fim do laco".into(),
        "decisao" => "rejeicao humana registrada; nada muda sem Go".into(),
        _ => "objetivo menor, ou modelo mais forte (--modelo)".into(),
    }
}

/// O relatorio em Markdown, gerado do registro (o JSON e a fonte; isto e so a vista).
pub fn relatorio(r: &Registro) -> String {
    let mut t = format!("# Evolucao {}\n\n", r.id);
    t.push_str(&format!("- **Estado:** {}\n", r.estado.nome()));
    if let Some(m) = &r.motivo {
        t.push_str(&format!(
            "- **Motivo:** {m}{}\n",
            r.etapa
                .as_deref()
                .map(|e| format!(" (etapa {e})"))
                .unwrap_or_default()
        ));
    }
    t.push_str(&format!("- **Item:** `{}` — {}\n", r.item, r.porque));
    t.push_str(&format!("- **Modelo que rodou:** {}\n", r.modelo));
    if let Some(n) = &r.nota_do_modelo {
        t.push_str(&format!("- **Nota do modelo:** {n}\n"));
    }
    t.push_str(&format!(
        "- **Repositorio:** {}  \n- **Base:** `{}`  \n- **Ramo:** `{}`{}\n",
        r.repositorio,
        r.base,
        r.ramo,
        match (&r.commit, r.estado) {
            (Some(c), Estado::EsperandoGo | Estado::Aprovado | Estado::Rejeitado) =>
                format!(" em `{c}`"),
            _ => " (nao nasceu)".into(),
        }
    ));
    if let Some(tarefa) = &r.tarefa {
        t.push_str(&format!("- **Tarefa do laco:** {tarefa}\n"));
    }
    t.push_str("\n## Plano\n\n");
    if r.plano.is_empty() {
        t.push_str("(sem plano)\n");
    }
    for (i, p) in r.plano.iter().enumerate() {
        t.push_str(&format!("{}. {p}\n", i + 1));
    }
    t.push_str("\n## Diff\n\n");
    if r.arquivos.is_empty() {
        t.push_str("(nenhum arquivo)\n");
    } else {
        let (a, d) = r
            .arquivos
            .iter()
            .fold((0, 0), |(a, d), f| (a + f.adicoes, d + f.remocoes));
        t.push_str(&format!(
            "{} arquivo(s), +{a} -{d}\n\n| arquivo | estado | + | - |\n|---|---|---:|---:|\n",
            r.arquivos.len()
        ));
        for f in &r.arquivos {
            t.push_str(&format!(
                "| `{}` | {} | {} | {} |\n",
                f.caminho, f.estado, f.adicoes, f.remocoes
            ));
        }
    }
    t.push_str("\n## Portoes\n\n");
    if r.portoes.is_empty() {
        t.push_str("(nenhum rodou)\n");
    } else {
        t.push_str("| portao | alvo | resultado | s | detalhe |\n|---|---|---|---:|---|\n");
        for p in &r.portoes {
            t.push_str(&format!(
                "| {} | `{}` | {} | {:.1} | {} |\n",
                p.nome,
                p.alvo,
                if p.verde { "verde" } else { "VERMELHO" },
                p.segundos,
                p.detalhe.replace('|', "\\|").replace('\n', " ")
            ));
        }
    }
    t.push_str("\n## Revisao do proprio diff\n\n");
    match &r.revisao {
        None => t.push_str("(nao rodou)\n"),
        Some(v) => {
            t.push_str(&format!("{}\n\n", v["resumo"].as_str().unwrap_or("")));
            let achados = v["achados"].as_array().cloned().unwrap_or_default();
            if achados.is_empty() {
                t.push_str("Sem achados.\n");
            }
            for a in achados {
                t.push_str(&format!(
                    "- **{}** `{}`:{} {}\n",
                    a["severidade"].as_str().unwrap_or(""),
                    a["arquivo"].as_str().unwrap_or(""),
                    a["linha"]
                        .as_u64()
                        .map(|n| n.to_string())
                        .unwrap_or_default(),
                    a["achado"].as_str().unwrap_or("")
                ));
            }
        }
    }
    t.push_str("\n## Proximo passo\n\n");
    match r.estado {
        Estado::EsperandoGo => t.push_str(&format!(
            "Revisar o ramo e decidir: `phxclaw evoluir aprovar {id}` (so marca e mostra o \
comando de merge) ou `phxclaw evoluir rejeitar {id} --motivo \"...\"`. Nada foi mesclado.\n",
            id = r.id
        )),
        Estado::Aprovado => t.push_str(&format!(
            "Aprovado. O merge e do humano: `{}`.\n",
            comando_de_merge(r)
        )),
        Estado::Rejeitado => t.push_str(&format!(
            "Rejeitado. O ramo ficou; apagar e do humano: `{}`.\n",
            comando_de_apagar(r)
        )),
        Estado::Vermelho | Estado::Recusado => t.push_str("Nada a decidir: o ramo nao nasceu.\n"),
    }
    t
}

/// Aspas simples de shell, sempre: o comando sai para o humano colar no terminal, e o
/// registro e um JSON em disco que qualquer processo do usuario reescreve. A validacao do
/// registro vem antes; a aspa e a segunda trava, e nao depende dela.
pub fn aspas_de_shell(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

pub fn comando_de_merge(r: &Registro) -> String {
    format!(
        "git -C {} merge --no-ff {}",
        aspas_de_shell(&r.repositorio),
        aspas_de_shell(&r.ramo)
    )
}

pub fn comando_de_apagar(r: &Registro) -> String {
    format!(
        "git -C {} branch -D {}",
        aspas_de_shell(&r.repositorio),
        aspas_de_shell(&r.ramo)
    )
}

/// Nome de item do backlog que pode virar ramo: letra ou digito no comeco, depois `_` e `-`.
fn item_valido(item: &str) -> bool {
    item.chars()
        .next()
        .is_some_and(|c| c.is_ascii_alphanumeric())
        && item
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// O registro e um JSON em `.phxclaw/` e volta do disco sem garantia nenhuma: tudo o que dele
/// vira argumento de git ou texto de comando se confere contra a FORMA que este modulo grava,
/// antes de qualquer git rodar (um `repositorio` trocado faria o git do hospedeiro abrir um
/// repositorio alheio, com o `core.fsmonitor` dele).
fn conferir_registro(projeto: &Path, id: &str, r: &Registro) -> Result<(), String> {
    let adulterado = |m: String| Err(format!("registro {id} adulterado, recusado: {m}"));
    if r.id != id {
        return adulterado(format!("o id gravado e {:?}", r.id));
    }
    if !item_valido(&r.item) {
        return adulterado(format!("item {:?} fora da forma", r.item));
    }
    // `<item>-AAAAMMDD-HHMMSS`, como `evoluir` o monta.
    let data =
        r.id.strip_prefix(r.item.as_str())
            .and_then(|x| x.strip_prefix('-'))
            .unwrap_or("");
    let forma_da_data = data.len() == 15
        && data.bytes().enumerate().all(|(i, b)| {
            if i == 8 {
                b == b'-'
            } else {
                b.is_ascii_digit()
            }
        });
    if !forma_da_data {
        return adulterado(format!("id {:?} nao e <item>-AAAAMMDD-HHMMSS", r.id));
    }
    if r.ramo != format!("{PREFIXO_DO_RAMO}{}", r.id) || crate::git::nome_de_ramo(&r.ramo).is_err()
    {
        return adulterado(format!("ramo {:?} nao e {PREFIXO_DO_RAMO}{}", r.ramo, r.id));
    }
    if let Some(c) = &r.commit
        && !(matches!(c.len(), 40 | 64) && c.bytes().all(|b| b.is_ascii_hexdigit()))
    {
        return adulterado(format!("commit {c:?} nao e um sha"));
    }
    let topo = git_hospedeiro(projeto, &["rev-parse", "--show-toplevel"])?;
    if r.repositorio != topo.trim() {
        return adulterado(format!(
            "repositorio {:?} nao e a raiz do projeto ({})",
            r.repositorio,
            topo.trim()
        ));
    }
    Ok(())
}

// ------------------------------------------------------------------ decisao humana

/// So MARCA aprovado e devolve o comando de merge para o humano rodar. Recusa se o ramo nao
/// apontar mais para o commit que os portoes conferiram: o Go vale para o que foi medido.
pub fn aprovar(projeto: &Path, id: &str) -> Result<(Registro, String), String> {
    let mut r = decidivel(projeto, id)?;
    r.estado = Estado::Aprovado;
    r.decidido_em = Some(Utc::now());
    gravar(projeto, &r)?;
    anotar_desfecho(projeto, &r, "aprovado")?;
    let cmd = comando_de_merge(&r);
    Ok((r, cmd))
}

/// Marca rejeitado; o ramo fica (apagar e do humano) e o motivo vira a causa do desfecho.
pub fn rejeitar(projeto: &Path, id: &str, motivo: Option<&str>) -> Result<Registro, String> {
    let mut r = decidivel(projeto, id)?;
    r.estado = Estado::Rejeitado;
    r.etapa = Some("decisao".into());
    r.motivo = Some(
        motivo
            .map(str::to_string)
            .unwrap_or_else(|| "rejeitado pelo humano sem motivo escrito".into()),
    );
    r.decidido_em = Some(Utc::now());
    gravar(projeto, &r)?;
    anotar_desfecho(projeto, &r, "rejeitado")?;
    Ok(r)
}

fn decidivel(projeto: &Path, id: &str) -> Result<Registro, String> {
    let r = carregar(projeto, id)?;
    conferir_registro(projeto, id, &r)?;
    if r.estado != Estado::EsperandoGo {
        return Err(format!(
            "evolucao {id} esta {}: so se decide a que espera Go",
            r.estado.nome()
        ));
    }
    let commit = r
        .commit
        .clone()
        .ok_or_else(|| format!("evolucao {id} sem commit registrado"))?;
    let atual = git_hospedeiro(
        Path::new(&r.repositorio),
        &[
            "rev-parse",
            "--verify",
            "--quiet",
            &format!("refs/heads/{}^{{commit}}", r.ramo),
        ],
    )
    .map_err(|_| format!("o ramo {} nao existe mais em {}", r.ramo, r.repositorio))?;
    if atual.trim() != commit {
        return Err(format!(
            "o ramo {} aponta para {} e os portoes conferiram {commit}: o Go vale so para o \
que foi medido",
            r.ramo,
            atual.trim()
        ));
    }
    Ok(r)
}

// ------------------------------------------------------------------ o ciclo

pub struct Ciclo {
    /// A pasta do projeto (onde moram a politica, o backlog e `.phxclaw/`).
    pub projeto: PathBuf,
    pub bwrap: PathBuf,
    /// `--item`: sem ele, o primeiro candidato.
    pub item: Option<String>,
    pub agora: DateTime<Utc>,
    pub nota_do_modelo: Option<String>,
    pub prazo_dos_portoes: Duration,
}

impl Ciclo {
    /// O ciclo com o relogio de agora, sem `--item` e com 1 h de prazo por portao (o `test`
    /// de um workspace grande passa de 20 min nesta maquina).
    pub fn agora(projeto: PathBuf, bwrap: PathBuf) -> Self {
        Self {
            projeto,
            bwrap,
            item: None,
            agora: Utc::now(),
            nota_do_modelo: None,
            prazo_dos_portoes: Duration::from_secs(3600),
        }
    }
}

/// Onde e por que o ciclo parou depois de comecar.
struct Parada {
    estado: Estado,
    etapa: String,
    motivo: String,
}

fn vermelho(etapa: &str, motivo: impl Into<String>) -> Parada {
    Parada {
        estado: Estado::Vermelho,
        etapa: etapa.into(),
        motivo: motivo.into(),
    }
}

/// Roda um ciclo inteiro. `Err` so ANTES de comecar (politica, item, repositorio, toolchain);
/// depois que o id existe, todo desfecho vira registro, relatorio e evidencia.
pub async fn evoluir(agente: &Agent, c: &Ciclo, obs: &dyn Observer) -> Result<Registro, String> {
    let pol = Politica::carregar(&c.projeto)?;
    let (item, porque) = escolher(&c.projeto, &pol, c.item.as_deref())?;
    let topo = git_hospedeiro(&c.projeto, &["rev-parse", "--show-toplevel"])?
        .trim()
        .to_string();
    let prefixo = git_hospedeiro(&c.projeto, &["rev-parse", "--show-prefix"])?
        .trim()
        .to_string();
    let rust = crate::sistema::RustProjectTool::detectar(c.bwrap.clone()).ok_or(
        "sem toolchain Rust no hospedeiro: os portoes nao rodam, e sem portao nao ha ramo",
    )?;
    let id = format!("{}-{}", item.nome, c.agora.format("%Y%m%d-%H%M%S"));
    validar_id(&id)?;
    let ramo = format!("{PREFIXO_DO_RAMO}{id}");
    crate::git::nome_de_ramo(&ramo).map_err(|e| e.to_string())?;
    if pasta(&c.projeto).join(format!("{id}.json")).exists() {
        return Err(format!("ja existe a evolucao {id}"));
    }
    if ramo_existe(Path::new(&topo), &ramo) {
        return Err(format!("o ramo {ramo} ja existe em {topo}"));
    }

    let store = &agente.store;
    let mut mae = Task::new(
        format!("evolucao {id}: item {}", item.nome),
        agente.llm.id(),
    );
    mae.mudar_estado(TaskStatus::Running);
    store.save(&mae).map_err(|e| e.to_string())?;
    let m = store.workdir(&mae.id);
    let mut r = Registro {
        id: id.clone(),
        item: item.nome.clone(),
        porque,
        criado_em: c.agora,
        modelo: agente.llm.id(),
        nota_do_modelo: c.nota_do_modelo.clone(),
        repositorio: topo.clone(),
        base: String::new(),
        ramo: ramo.clone(),
        commit: None,
        estado: Estado::Vermelho,
        etapa: None,
        motivo: None,
        plano: vec![],
        tarefa: None,
        arquivos: vec![],
        portoes: vec![],
        revisao: None,
        decidido_em: None,
    };
    let parada = etapas(
        agente, c, &pol, &item, &prefixo, &rust, &mae, &m, &mut r, obs,
    )
    .await;
    // O clone (com o `target/` dos portoes) sai sempre: o ramo ja esta no produto, ou nao
    // deve nascer. A pasta da tarefa-mae fica, com o registro dela.
    let _ = std::fs::remove_dir_all(m.join("repo"));
    let _ = std::fs::remove_file(m.join("evolucao.bundle"));
    match parada {
        None => r.estado = Estado::EsperandoGo,
        Some(p) => {
            r.estado = p.estado;
            r.etapa = Some(p.etapa);
            r.motivo = Some(p.motivo);
        }
    }
    // Pelo ponto unico (`Task::mudar_estado`): a transicao final entra no historico, senao a
    // trilha do Kanban perde o ultimo salto desta tarefa-mae de evolucao.
    let desfecho = if r.estado == Estado::EsperandoGo {
        TaskStatus::Completed
    } else {
        TaskStatus::Failed
    };
    mae.mudar_estado(desfecho);
    mae.error = r.motivo.clone();
    let _ = store.save(&mae);
    gravar(&c.projeto, &r)?;
    let desfecho = match r.estado {
        Estado::EsperandoGo => "verde",
        Estado::Recusado => "recusado",
        _ => "vermelho",
    };
    anotar_desfecho(&c.projeto, &r, desfecho)?;
    Ok(r)
}

/// O miolo do ciclo. `None` = tudo verde e o ramo nasceu no produto.
#[allow(clippy::too_many_arguments)]
async fn etapas(
    agente: &Agent,
    c: &Ciclo,
    pol: &Politica,
    item: &Item,
    prefixo: &str,
    rust: &crate::sistema::RustProjectTool,
    mae: &Task,
    m: &Path,
    r: &mut Registro,
    obs: &dyn Observer,
) -> Option<Parada> {
    // 1. Clone raso do commit do produto, no hospedeiro: quem roda e o `upload-pack` do
    // PRODUTO (confiavel), e o git roda com `-C` no produto e nao na pasta da tarefa -- assim
    // nenhum `.git` que exista acima dela entra na descoberta. O destino vai absoluto porque
    // o `-C` mudaria a base de um caminho relativo para dentro do produto. Dai em diante, so
    // git no sandbox -- inclusive o primeiro `rev-parse` do clone.
    if let Err(e) = std::fs::create_dir_all(m) {
        return Some(vermelho("clone", e.to_string()));
    }
    let clone = match std::path::absolute(m.join("repo")) {
        Ok(c) => c,
        Err(e) => return Some(vermelho("clone", e.to_string())),
    };
    let url = format!("file://{}", r.repositorio);
    if let Err(e) = git_hospedeiro(
        Path::new(&r.repositorio),
        &[
            "clone",
            "-q",
            "--no-checkout",
            "--depth",
            "1",
            "--no-tags",
            &url,
            &clone.to_string_lossy(),
        ],
    ) {
        return Some(vermelho("clone", e));
    }
    r.base = match git_no_sandbox(c, m, "repo", vec!["rev-parse".into(), "HEAD".into()]).await {
        Ok(b) => b.trim().to_string(),
        Err(e) => return Some(vermelho("clone", e)),
    };
    let ctx = ToolContext {
        task_id: mae.id.clone(),
        workdir: m.to_path_buf(),
        timeout: c.prazo_dos_portoes,
    };

    // 2. A worktree pela ferramenta de sempre, no ramo do ciclo (que so existe no clone).
    let arvore = WorktreeTool {
        bwrap: c.bwrap.clone(),
        timeout: PRAZO_DO_GIT,
    };
    let v = match json_de(
        &arvore,
        json!({"action":"add","path":"repo","name":r.id,"branch":r.ramo}),
        &ctx,
    )
    .await
    {
        Ok(v) => v,
        Err(e) => return Some(vermelho("worktree", e)),
    };
    let p = v["path"].as_str().unwrap_or_default().to_string();
    let projeto_na_arvore = juntar(&p, prefixo);

    // 3. Plano e execucao pelo laco normal, com a pasta da filha ligada a worktree (o modelo
    // de `nuvem.rs`). A configuracao e a de subagente com o teto de passos do pai: nada
    // concedido a mais, e sem `ask_user` (ninguem acompanha um ciclo agendado).
    let mut filha = Task::new(objetivo(item, &r.ramo), agente.llm.id());
    filha.parent = Some(mae.id.clone());
    r.tarefa = Some(filha.id.clone());
    if let Err(e) = crate::nuvem::ligar_pasta(&agente.store, &filha.id, &m.join(&projeto_na_arvore))
    {
        return Some(vermelho("worktree", e.to_string()));
    }
    let sub = Agent::new(
        agente.llm.clone(),
        agente.tools.clone(),
        config_do_ciclo(&agente.config, contexto(item, pol)),
        agente.store.clone(),
    );
    if let Err(e) = sub.plan(&mut filha).await {
        return Some(vermelho("plano", e));
    }
    r.plano = filha.plan.clone();
    let filha = sub.run(filha, &CancelFlag::default(), obs).await;
    for t in &sub.tools {
        t.finish(&filha.id).await;
    }
    if filha.status != TaskStatus::Completed {
        return Some(vermelho(
            "laco",
            format!(
                "o laco terminou em {:?}: {}",
                filha.status,
                filha.error.clone().unwrap_or_default()
            ),
        ));
    }

    // 4. Commit pela mae (a filha nao ve o .git), pelo `git_write` de sempre: a varredura de
    // segredos do `add` vale aqui tambem.
    let git = GitTool::escrita(c.bwrap.clone());
    let st = match json_de(&git, json!({"action":"add","path":p,"paths":["."]}), &ctx).await {
        Ok(v) => v,
        Err(e) => return Some(vermelho("commit", e)),
    };
    if st["limpo"].as_bool() == Some(true) {
        return Some(vermelho(
            "commit",
            "o laco terminou sem mudar nenhum arquivo",
        ));
    }
    let msg = format!("evolucao {}: item {}", r.id, item.nome);
    match json_de(
        &git,
        json!({"action":"commit","path":p,"message":msg}),
        &ctx,
    )
    .await
    {
        Ok(v) => r.commit = v["commit"]["commit"].as_str().map(str::to_string),
        Err(e) => return Some(vermelho("commit", e)),
    }
    let Some(commit) = r.commit.clone() else {
        return Some(vermelho("commit", "o git nao devolveu o commit"));
    };

    // 5. Portao do alcance, ANTES de gastar compilacao: o diff inteiro contra a politica.
    let raw = match git_no_sandbox(
        c,
        m,
        &p,
        vec![
            "diff".into(),
            "--raw".into(),
            "--no-renames".into(),
            "--no-relative".into(),
            "--no-abbrev".into(),
            "-z".into(),
            r.base.clone(),
            commit.clone(),
        ],
    )
    .await
    {
        Ok(s) => s,
        Err(e) => return Some(vermelho("alcance", e)),
    };
    // O conteudo do MESMO par de commits, para o mesmo motor. `--text` mostra ate o que tem
    // NUL (um `-diff` do `.gitattributes` nao esconde nada); sem textconv nem diff externo,
    // e com os prefixos ditos aqui, nao pela configuracao do clone.
    let conteudo = match git_no_sandbox(
        c,
        m,
        &p,
        [
            "diff",
            "--no-color",
            "--no-ext-diff",
            "--no-textconv",
            "--no-renames",
            "--no-relative",
            "--full-index",
            "--text",
            "-U0",
            "--src-prefix=a/",
            "--dst-prefix=b/",
        ]
        .iter()
        .map(|s| s.to_string())
        .chain([r.base.clone(), commit.clone()])
        .collect(),
    )
    .await
    {
        Ok(s) => s,
        Err(e) => return Some(vermelho("alcance", e)),
    };
    let mudancas = analisar_raw(&raw);
    let recusas = conferir_alcance(pol, prefixo, &mudancas, &conteudo);
    let diff = match diff_do_repo(&c.bwrap, m, &p, Some(&r.base), false, &[], PRAZO_DO_GIT).await {
        Ok(d) => d,
        Err(e) => return Some(vermelho("alcance", e.to_string())),
    };
    r.arquivos = analisar_diff(&diff)
        .into_iter()
        .map(|a| ArquivoMudado {
            caminho: a.caminho,
            estado: a.estado,
            adicoes: a.adicoes,
            remocoes: a.remocoes,
        })
        .collect();
    if !recusas.is_empty() {
        return Some(Parada {
            estado: Estado::Recusado,
            etapa: "alcance".into(),
            motivo: recusas.join("; "),
        });
    }

    // 6. Os portoes, no sandbox do `rust_project`: fmt na raiz do projeto, clippy e testes
    // em cada crate tocado. O primeiro vermelho para o ciclo.
    let alvos = crates_tocados(&m.join(&projeto_na_arvore), prefixo, &mudancas);
    for nome in &pol.portoes {
        let lista: Vec<String> = if nome == "fmt" {
            vec![String::new()]
        } else {
            alvos.clone()
        };
        for alvo in lista {
            let caminho = juntar(&projeto_na_arvore, &alvo);
            let inicio = Instant::now();
            let (verde, detalhe) =
                match json_de(rust, json!({"action":nome,"path":caminho}), &ctx).await {
                    Ok(v) => veredito(nome, &v),
                    Err(e) => (false, e),
                };
            r.portoes.push(Portao {
                nome: nome.clone(),
                alvo: if alvo.is_empty() {
                    ".".into()
                } else {
                    alvo.clone()
                },
                verde,
                segundos: inicio.elapsed().as_secs_f64(),
                detalhe: detalhe.clone(),
            });
            if !verde {
                return Some(vermelho(
                    &format!("portao:{nome}"),
                    format!(
                        "{nome} em {}: {detalhe}",
                        if alvo.is_empty() { "." } else { &alvo }
                    ),
                ));
            }
        }
    }

    // 7. Revisao do proprio diff pelo motor do `code_review`. Revisao que falha e vermelho:
    // a rede so endurece.
    let rev = match crate::revisao::revisar(
        agente.llm.as_ref(),
        &diff,
        Some("auto-evolucao: o diff faz o que o item pede, e o teste prova?"),
    )
    .await
    {
        Ok(v) => v,
        Err(e) => return Some(vermelho("revisao", e)),
    };
    r.revisao = serde_json::to_value(&rev).ok();
    if rev.tem_ao_menos(&pol.revisao_bloqueia_em) {
        return Some(vermelho(
            "revisao",
            format!(
                "a revisao achou defeito de severidade {} ou maior",
                pol.revisao_bloqueia_em
            ),
        ));
    }

    // 8. Tudo verde: o ramo nasce no produto por bundle, e so ele (nada de merge, nada no
    // ramo atual).
    if let Err(e) = git_no_sandbox(
        c,
        m,
        "repo",
        vec![
            "bundle".into(),
            "create".into(),
            "/work/evolucao.bundle".into(),
            format!("{}..{}", r.base, r.ramo),
        ],
    )
    .await
    {
        return Some(vermelho("exportar", e));
    }
    if let Err(e) = buscar_ramo(
        Path::new(&r.repositorio),
        &m.join("evolucao.bundle"),
        &r.ramo,
    ) {
        return Some(vermelho("exportar", e));
    }
    match git_hospedeiro(
        Path::new(&r.repositorio),
        &["rev-parse", &format!("refs/heads/{}", r.ramo)],
    ) {
        Ok(s) if s.trim() == commit => None,
        Ok(s) => Some(vermelho(
            "exportar",
            format!(
                "o ramo nasceu em {} e nao no commit conferido {commit}",
                s.trim()
            ),
        )),
        Err(e) => Some(vermelho("exportar", e)),
    }
}

/// Subagente com o teto de passos do pai: `config_de_subagente` tira o que abre outro
/// agente e o `ask_user`; nada se acrescenta.
fn config_do_ciclo(pai: &AgentConfig, contexto: String) -> AgentConfig {
    let extra = match &pai.extra_instructions {
        Some(x) => format!("{x}\n\n{contexto}"),
        None => contexto,
    };
    AgentConfig {
        max_steps: pai.max_steps,
        extra_instructions: Some(extra),
        ..config_de_subagente(pai)
    }
}

/// O objetivo da filha. So o nome do item e o pedido: o motor le nome de arquivo no
/// objetivo como arquivo a entregar (`arquivos_pedidos`), entao evidencia e caminhos vetados
/// vao pelas instrucoes do sistema (`contexto`), nunca por aqui.
fn objetivo(item: &Item, ramo: &str) -> String {
    format!(
        "Auto-evolucao do PhxClaw, item do backlog «{}» (estado {}). Faca UM passo pequeno que \
avance este item, em ferramenta ou teste, e escreva o teste que o prova. Sua pasta de trabalho e \
uma copia isolada do projeto no ramo {ramo}: edite os arquivos aqui e nao rode git -- o commit e \
feito por quem te chamou quando voce terminar.",
        item.nome, item.estado
    )
}

/// O que a filha precisa saber e nao pode estar no objetivo.
fn contexto(item: &Item, pol: &Politica) -> String {
    format!(
        "AUTO-EVOLUCAO. O que existe hoje do item {}: {}\n\
So pode tocar estes caminhos do projeto (lista de permissao; qualquer outro, e um portao \
recusa o diff inteiro): {}. Nada de Cargo.toml, build.rs, .cargo/, #[path], include!/env!, linha \
mod, arquivo apagado ou executavel; fora de teste, so o std puro (sem std::process, std::fs, \
crate:: ou outro crate).\n\
Depois do fim rodam os portoes {} e uma revisao do diff; so com tudo verde o ramo nasce, e ele \
espera o Go humano.",
        item.nome,
        if item.evidencia.is_empty() {
            "(sem evidencia registrada)"
        } else {
            &item.evidencia
        },
        pol.permitidos.join(", "),
        pol.portoes.join(", ")
    )
}

/// O veredito de um portao a partir da saida do `rust_project`. Clippy so e verde com zero
/// avisos: e o portao da casa.
fn veredito(nome: &str, v: &Value) -> (bool, String) {
    let sucesso = v["sucesso"].as_bool() == Some(true);
    let avisos = v["avisos"].as_u64().unwrap_or(0);
    let erros = v["erros"].as_u64().unwrap_or(0);
    let verde = sucesso && (nome != "clippy" || avisos == 0);
    let mut d = format!("{erros} erro(s), {avisos} aviso(s)");
    if let Some(t) = v["testes"].as_array().filter(|t| !t.is_empty()) {
        let linhas: Vec<&str> = t.iter().filter_map(Value::as_str).take(4).collect();
        d.push_str(&format!("; {}", linhas.join(" / ")));
    }
    if !verde {
        if let Some(x) = v["diagnosticos"].as_array().and_then(|a| a.first()) {
            d.push_str(&format!(
                "; {}:{} {}",
                x["arquivo"].as_str().unwrap_or(""),
                x["linha"].as_u64().unwrap_or(0),
                x["mensagem"]
                    .as_str()
                    .unwrap_or("")
                    .lines()
                    .next()
                    .unwrap_or("")
            ));
        }
        let cauda = v["stderr_cauda"].as_str().unwrap_or("").trim();
        if !cauda.is_empty() {
            let c: String = cauda
                .chars()
                .rev()
                .take(300)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect();
            d.push_str(&format!("; {c}"));
        }
    }
    (verde, d)
}

/// As pastas de crate (relativas ao projeto) dos arquivos mudados: o `Cargo.toml` com
/// `[package]` mais proximo; sem nenhum, a raiz do projeto ("").
fn crates_tocados(raiz: &Path, prefixo: &str, mudancas: &[Mudanca]) -> Vec<String> {
    let mut v: Vec<String> = Vec::new();
    for m in mudancas {
        let rel = m.caminho.strip_prefix(prefixo).unwrap_or(&m.caminho);
        let mut dir = Path::new(rel).parent();
        let achado = loop {
            let Some(d) = dir else { break String::new() };
            let toml = raiz.join(d).join("Cargo.toml");
            // Le so arquivo regular: o agente escreveu nesta arvore, e link aqui levaria o
            // hospedeiro a ler fora dela.
            let regular = std::fs::symlink_metadata(&toml).is_ok_and(|x| x.file_type().is_file());
            if regular && std::fs::read_to_string(&toml).is_ok_and(|t| t.contains("[package]")) {
                break d.to_string_lossy().to_string();
            }
            dir = d.parent();
        };
        if !v.contains(&achado) {
            v.push(achado);
        }
    }
    v.sort();
    v
}

fn juntar(a: &str, b: &str) -> String {
    let b = b.trim_matches('/');
    match (a.is_empty(), b.is_empty()) {
        (_, true) => a.to_string(),
        (true, false) => b.to_string(),
        _ => format!("{}/{b}", a.trim_end_matches('/')),
    }
}

async fn json_de(t: &dyn Tool, args: Value, ctx: &ToolContext) -> Result<Value, String> {
    let o = t.run(args, ctx).await.map_err(|e| e.to_string())?;
    serde_json::from_str(&o.content).map_err(|e| format!("{}: {e}", t.spec().name))
}

async fn git_no_sandbox(
    c: &Ciclo,
    m: &Path,
    repo: &str,
    args: Vec<String>,
) -> Result<String, String> {
    let s = rodar_git(&c.bwrap, m, repo, args, PRAZO_DO_GIT)
        .await
        .map_err(|e| e.to_string())?;
    // Saida cortada e diff pela metade: o portao do alcance conferiria so o comeco dele.
    if s.truncated {
        return Err(
            "git: saida maior que o teto, cortada (o diff nao se confere pela metade)".into(),
        );
    }
    if s.exit_code == Some(0) {
        Ok(s.stdout)
    } else {
        Err(format!(
            "git (saida {:?}): {}",
            s.exit_code,
            s.stderr.trim()
        ))
    }
}

/// O git do hospedeiro, so no repositorio do PRODUTO (confiavel): `rev-parse` da raiz e das
/// refs, o `clone` que faz nascer o clone e o `fetch` do bundle. Nunca no clone -- o agente
/// escreveu la, e o `.git/config` dele pode declarar filtro, textconv ou fsmonitor. O
/// subcomando se confere AQUI, e nao em quem chama: um `git_hospedeiro` novo com outro verbo
/// cai antes de criar o processo. A catraca de `tests/guardas.rs` declara esta porta em
/// `FORA_DO_BWRAP` e confere o primeiro argumento de cada chamada.
fn git_hospedeiro(dir: &Path, args: &[&str]) -> Result<String, String> {
    const VERBOS: &[&str] = &["rev-parse", "clone", "fetch"];
    let verbo = args.first().copied().unwrap_or("");
    if !VERBOS.contains(&verbo) {
        return Err(format!(
            "git {verbo:?} recusado no hospedeiro: so {VERBOS:?} no repositorio do produto"
        ));
    }
    let o = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .map_err(|e| format!("git: {e}"))?;
    if o.status.success() {
        Ok(String::from_utf8_lossy(&o.stdout).into_owned())
    } else {
        Err(format!(
            "git {}: {}",
            args.first().copied().unwrap_or(""),
            String::from_utf8_lossy(&o.stderr).trim()
        ))
    }
}

fn ramo_existe(repo: &Path, ramo: &str) -> bool {
    git_hospedeiro(
        repo,
        &[
            "rev-parse",
            "--verify",
            "--quiet",
            &format!("refs/heads/{ramo}"),
        ],
    )
    .is_ok()
}

/// A UNICA escrita no repositorio do produto: uma ref nova `refs/heads/evolucao/...` a partir
/// do bundle. Sem `+` (nunca sobrescreve) e com o prefixo conferido aqui, nao por quem chama.
fn buscar_ramo(repo: &Path, bundle: &Path, ramo: &str) -> Result<(), String> {
    if !ramo.starts_with(PREFIXO_DO_RAMO) || ramo_existe(repo, ramo) {
        return Err(format!(
            "ramo {ramo} recusado: fora de {PREFIXO_DO_RAMO} ou ja existe"
        ));
    }
    let refspec = format!("refs/heads/{ramo}:refs/heads/{ramo}");
    git_hospedeiro(
        repo,
        &[
            "fetch",
            "-q",
            "--no-tags",
            "--no-write-fetch-head",
            &bundle.to_string_lossy(),
            &refspec,
        ],
    )
    .map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pol(vetados: &[&str]) -> Politica {
        Politica {
            backlog: "b.json".into(),
            permitidos: PERMISSAO_MAXIMA.iter().map(|s| s.to_string()).collect(),
            vetados: vetados.iter().map(|s| s.to_string()).collect(),
            itens_vetados: vec![],
            portoes: vec!["fmt".into()],
            revisao_bloqueia_em: "alta".into(),
        }
    }

    fn versionada() -> Politica {
        let p: Politica =
            serde_json::from_str(include_str!("../../../config/evolucao-politica.json")).unwrap();
        p.validar().unwrap()
    }

    /// Um diff de arquivos NOVOS (ou com o estado/modo dados), no par `--raw` + unificado que
    /// o ciclo le, com o projeto em `phxclaw/`.
    fn diff(arqs: &[(&str, &str, char, &str)]) -> (Vec<Mudanca>, String) {
        let mut raw = String::new();
        let mut uni = String::new();
        for (rel, modo, estado, corpo) in arqs {
            let c = format!("phxclaw/{rel}");
            raw.push_str(&format!(":000000 {modo} 0 1 {estado}\0{c}\0"));
            let linhas: Vec<&str> = corpo.lines().collect();
            uni.push_str(&format!(
                "diff --git a/{c} b/{c}\nnew file mode {modo}\nindex 0..1\n--- /dev/null\n+++ b/{c}\n@@ -0,0 +1,{} @@\n",
                linhas.len()
            ));
            for l in linhas {
                uni.push_str(&format!("+{l}\n"));
            }
        }
        (analisar_raw(&raw), uni)
    }

    fn recusas(arqs: &[(&str, &str, char, &str)]) -> Vec<String> {
        let (m, u) = diff(arqs);
        conferir_alcance(&versionada(), "phxclaw/", &m, &u)
    }

    /// Um .rs novo, 100644.
    fn so(rel: &str, corpo: &str) -> Vec<String> {
        recusas(&[(rel, "100644", 'A', corpo)])
    }

    const TESTE: &str = "crates/phxclaw-agent/tests/calculadora_x.rs";
    const CALC: &str = "crates/phxclaw-agent/src/calculadora.rs";

    #[test]
    fn casa_pasta_arquivo_e_estrela_sem_atravessar_pasta() {
        assert!(casa(
            "crates/phxclaw-sandbox/",
            "crates/phxclaw-sandbox/src/lib.rs"
        ));
        assert!(!casa(
            "crates/phxclaw-sandbox/",
            "crates/phxclaw-sandbox-x/src/lib.rs"
        ));
        assert!(casa("crates/a/src/motor.rs", "crates/a/src/motor.rs"));
        assert!(!casa("crates/a/src/motor.rs", "crates/a/src/motor.rs.bak"));
        // sem barra final, o nome tambem veta a pasta com esse nome
        assert!(casa("deploy", "deploy/k8s/x.yaml"));
        assert!(casa(
            "config/capabilities-*.json",
            "config/capabilities-v057.json"
        ));
        assert!(!casa(
            "config/capabilities-*.json",
            "config/capabilities-x/y.json"
        ));
        assert!(casa("LICENSE*", "LICENSE-MIT"));
        assert!(!casa("Cargo.toml", "crates/a/Cargo.toml"));
    }

    #[test]
    fn permissao_e_estreita_e_o_teto_so_cobre_o_que_cabe() {
        let t = "crates/*/tests/**";
        assert!(permite(t, "crates/a/tests/x.rs"));
        assert!(permite(t, "crates/a/tests/dados/y.json"));
        assert!(!permite(t, "crates/a/tests"));
        assert!(!permite(t, "crates/a/src/x.rs"));
        assert!(!permite(t, "crates/a/b/tests/x.rs"));
        // literal e o arquivo exato, nunca a pasta com esse nome
        assert!(permite(CALC, CALC));
        assert!(!permite(CALC, &format!("{CALC}/x.rs")));
        for ok in [
            t,
            "crates/phxclaw-agent/tests/**",
            "crates/x/tests/a.rs",
            "crates/x/tests/*.rs",
        ] {
            assert!(cabe(ok, t), "{ok}");
        }
        for frouxo in [
            "**",
            "crates/**",
            "crates/*/**",
            "crates/*/src/**",
            "crates/x/tests",
            "crates/**/tests/**",
        ] {
            assert!(!cabe(frouxo, t), "{frouxo}");
        }
        assert!(cabe(CALC, CALC));
        assert!(!cabe("crates/*/src/calculadora.rs", CALC));
        assert!(!cabe("crates/phxclaw-agent/src/lib.rs", CALC));
    }

    #[test]
    fn a_politica_versionada_e_lista_de_permissao() {
        let p = versionada();
        for c in [TESTE, CALC, "crates/phxclaw-office/tests/a/b.rs"] {
            assert!(p.permite(c), "{c} deveria estar permitido");
        }
        for c in [
            "crates/phxclaw-agent/src/lib.rs",
            "crates/phxclaw-agent/src/motor.rs",
            "crates/phxclaw-agent/src/instrucoes.rs",
            "crates/phxclaw-agent/src/git.rs",
            "crates/phxclaw-agent/src/sistema.rs",
            "crates/phxclaw-agent/src/equipe.rs",
            "crates/phxclaw-agent/src/ferramentas.rs",
            "crates/phxclaw-agent/src/fluxo_http.rs",
            "crates/phxclaw-agent/src/pwa.rs",
            "crates/phxclaw-agent/src/evolucao.rs",
            "crates/phxclaw-agent/Cargo.toml",
            "crates/phxclaw-agent/build.rs",
            "config/agents/x.agent.json",
            "config/evolucao-politica.json",
            "apps/phxclaw-ui/assets/tarefas.js",
            "Cargo.toml",
            ".cargo/config.toml",
        ] {
            assert!(!p.permite(c), "{c} deveria estar negado");
        }
        // o teto do codigo nunca cobre a propria cerca
        for c in [POLITICA, "crates/phxclaw-agent/src/evolucao.rs"] {
            assert!(!PERMISSAO_MAXIMA.iter().any(|m| permite(m, c)), "{c}");
        }
    }

    /// A3 (politica afrouxada): o minimo vive no codigo; a politica que permite mais que o
    /// teto e recusada INTEIRA. RED: sem a conferencia `cabe` em `validar`, a politica com
    /// `src/**` carrega e o `lib.rs` passa a ser permitido.
    #[test]
    fn politica_mais_frouxa_que_o_minimo_e_recusada_inteira() {
        for frouxo in [
            "crates/phxclaw-agent/src/lib.rs",
            "crates/phxclaw-agent/src/**",
            "crates/**",
            "**",
            "Cargo.toml",
        ] {
            let mut p = pol(&[]);
            p.permitidos.push(frouxo.into());
            let e = p.validar().unwrap_err();
            assert!(e.contains("mais frouxo") && e.contains(frouxo), "{e}");
        }
        let mut vazio = pol(&[]);
        vazio.permitidos.clear();
        assert!(vazio.validar().is_err());
        // O JSON sem `permitidos` nao carrega: politica velha (lista de proibicoes) falha
        // fechado em vez de virar «tudo permitido».
        let velha =
            r#"{"backlog":"b","vetados":["x/"],"portoes":["fmt"],"revisao_bloqueia_em":"alta"}"#;
        assert!(serde_json::from_str::<Politica>(velha).is_err());
        let ok = pol(&[]);
        assert!(ok.clone().validar().is_ok());
        let mut sem_portao = ok.clone();
        sem_portao.portoes.clear();
        assert!(sem_portao.validar().is_err());
        let mut fuga = ok;
        fuga.vetados.push("../fora".into());
        assert!(fuga.validar().is_err());
        // e mesmo uma Politica montada sem validar nao permite fora do teto
        let mut sem_validar = pol(&[]);
        sem_validar.permitidos = vec!["**".into()];
        assert!(!sem_validar.permite("crates/phxclaw-agent/src/lib.rs"));
    }

    /// Os caminhos que a revisao de 09/10 (A2) mediu VERDES na lista de proibicoes: cada um
    /// recusado, com o caminho na mensagem.
    #[test]
    fn a2_caminhos_fora_da_permissao_sao_recusados_com_o_caminho() {
        let casos: &[(&str, &str)] = &[
            ("crates/phxclaw-agent/build.rs", "fn main() {}\n"),
            ("crates/x/Cargo.toml", "[package]\nname = \"x\"\n"),
            (".cargo/config.toml", "[target.x]\nrunner = \"sh\"\n"),
            (
                "crates/phxclaw-agent/src/instrucoes.rs",
                "pub fn nada() {}\n",
            ),
            ("crates/phxclaw-agent/src/git.rs", "pub fn nada() {}\n"),
            ("crates/phxclaw-agent/src/sistema.rs", "pub fn nada() {}\n"),
            ("crates/phxclaw-agent/src/equipe.rs", "pub fn nada() {}\n"),
            (
                "crates/phxclaw-agent/src/ferramentas.rs",
                "pub fn nada() {}\n",
            ),
            (
                "crates/phxclaw-agent/src/fluxo_http.rs",
                "pub fn nada() {}\n",
            ),
            ("crates/phxclaw-agent/src/pwa.rs", "pub fn nada() {}\n"),
            ("crates/phxclaw-agent/src/regras2.rs", "pub fn nada() {}\n"),
            ("config/agents/x.agent.json", "{}\n"),
            ("apps/phxclaw-ui/assets/tarefas.js", "go()\n"),
            // dentro de tests/, mas nome que muda o cargo ou pasta oculta
            ("crates/phxclaw-agent/tests/Cargo.toml", "[package]\n"),
            ("crates/phxclaw-agent/tests/build.rs", "fn main() {}\n"),
            ("crates/phxclaw-agent/tests/.cargo/config.toml", "x\n"),
            ("crates/phxclaw-agent/tests/rust-toolchain.toml", "x\n"),
            // os crates e testes de seguranca, no minimo do codigo
            ("crates/phxclaw-sandbox/tests/x.rs", "#[test]\nfn t() {}\n"),
            (
                "crates/phxclaw-agent/tests/guardas.rs",
                "#[test]\nfn t() {}\n",
            ),
            (
                "crates/phxclaw-agent/tests/evolucao.rs",
                "#[test]\nfn t() {}\n",
            ),
        ];
        for (rel, corpo) in casos {
            let r = so(rel, corpo);
            assert!(
                r.len() == 1 && r[0].starts_with(&format!("{rel}: ")),
                "{rel}: {r:?}"
            );
        }
        // lib.rs com `#[path]`: recusado pelo caminho E pelo conteudo
        let r = so(
            "crates/phxclaw-agent/src/lib.rs",
            "#[path = \"regras2.rs\"]\npub mod regras;\n",
        );
        assert!(r[0].contains("fora da lista de permissao"), "{r:?}");
    }

    /// Dentro do permitido, o conteudo. RED: com `recusas_do_conteudo` devolvendo vazio, todos
    /// estes passam (sao arquivos de teste ou a calculadora, que o caminho aceita).
    #[test]
    fn conteudo_recusado_mesmo_dentro_do_permitido() {
        let casos: &[(&str, &str, &str)] = &[
            (TESTE, "#[path = \"../src/regras.rs\"]\nmod r;\n", "#[path]"),
            (TESTE, "# [ r#path = \"x.rs\" ]\nmod r;\n", "#[path]"),
            (
                TESTE,
                "#[cfg_attr(all(), path = \"x.rs\")]\nmod r;\n",
                "cfg_attr",
            ),
            (
                TESTE,
                "const C: &str = include_str!(\"../../config/constitution.json\");\n",
                "include_str!",
            ),
            (
                TESTE,
                "const C: &[u8] = include_bytes!(\"x\");\n",
                "include_bytes!",
            ),
            (TESTE, "include!(\"x.rs\");\n", "include!"),
            (TESTE, "const H: &str = env!(\"HOME\");\n", "env!"),
            (
                TESTE,
                "const H: Option<&str> = option_env!(\"K\");\n",
                "option_env!",
            ),
            (TESTE, "extern crate alloc;\n", "extern crate"),
            (TESTE, "#[proc_macro]\npub fn m() {}\n", "proc_macro"),
            (TESTE, "#[no_mangle]\npub fn malloc() {}\n", "no_mangle"),
            (TESTE, "#[link(name = \"x\")]\n", "#[link]"),
            (TESTE, "macro_rules! m { () => {} }\n", "macro_rules"),
            (CALC, "use std::process::Command;\n", "std::process"),
            (
                CALC,
                "fn f() { std::process::Command::new(\"sh\"); }\n",
                "Command",
            ),
            (
                CALC,
                "fn f() { let _ = std::fs::read(\"/x\"); }\n",
                "std::fs",
            ),
            (
                CALC,
                "fn f() { let _ = ::std::fs::read(\"/x\"); }\n",
                "caminho absoluto",
            ),
            (CALC, "fn f() { crate::git::nada(); }\n", "crate::"),
            (CALC, "fn f() { super::git::nada(); }\n", "super::"),
            (CALC, "use phxclaw_types::arquivo;\n", "phxclaw_types::"),
            (CALC, "use std::{fs, fmt};\n", "std::{"),
            (
                CALC,
                "fn f(c: &ToolContext) { c.workdir.read_dir(); }\n",
                "read_dir",
            ),
            (CALC, "mod extra;\n", "mod declarado"),
            (CALC, "fn f() { unsafe {} }\n", "unsafe"),
        ];
        for (rel, corpo, motivo) in casos {
            let r = so(rel, corpo);
            assert!(
                !r.is_empty() && r.iter().all(|x| x.contains("conteudo recusado")),
                "{rel} {corpo:?}: {r:?}"
            );
            assert!(
                r.iter().any(|x| x.contains(motivo)),
                "{corpo:?} sem {motivo:?}: {r:?}"
            );
        }
        // `mod` em lib.rs/main.rs/mod.rs, entrando OU saindo, mesmo em pasta de teste
        let r = so("crates/phxclaw-agent/tests/comum/mod.rs", "pub mod novo;\n");
        assert!(r.iter().any(|x| x.contains("linha mod")), "{r:?}");
    }

    #[test]
    fn modo_apagar_binario_e_conteudo_que_nao_bate_sao_recusados() {
        let r = recusas(&[(TESTE, "100755", 'A', "#[test]\nfn t() {}\n")]);
        assert!(r[0].contains("modo executavel"), "{r:?}");
        let r = recusas(&[(TESTE, "100644", 'M', "")]);
        assert!(r.is_empty(), "{r:?}");
        let (m, _) = diff(&[(TESTE, "000000", 'D', "")]);
        assert!(conferir_alcance(&versionada(), "phxclaw/", &m, "")[0].contains("apagar"));
        let (m, _) = diff(&[(TESTE, "120000", 'A', "x")]);
        assert!(conferir_alcance(&versionada(), "phxclaw/", &m, "")[0].contains("link"));
        // o --raw diz um arquivo e o conteudo nao o traz: nao se aprova o que nao se leu
        let (m, _) = diff(&[(TESTE, "100644", 'A', "x")]);
        let r = conferir_alcance(&versionada(), "phxclaw/", &m, "");
        assert!(r[0].contains("sem conteudo"), "{r:?}");
        // o conteudo traz um arquivo que o --raw nao lista
        let (_, u) = diff(&[(TESTE, "100644", 'A', "x")]);
        let r = conferir_alcance(&versionada(), "phxclaw/", &[], &u);
        assert!(r[0].contains("fora do --raw"), "{r:?}");
        let (m, mut u) = diff(&[(TESTE, "100644", 'A', "")]);
        u.push_str("Binary files /dev/null and b/x differ\n");
        let r = conferir_alcance(&versionada(), "phxclaw/", &m, &u);
        assert!(r[0].contains("binario"), "{r:?}");
        let r = recusas(&[("crates/a/tests/a b.rs", "100644", 'A', "x")]);
        assert!(r[0].contains("fora de [A-Za-z0-9._/-]"), "{r:?}");
        let r = recusas(&[("../fora/tests/x.rs", "100644", 'A', "x")]);
        assert!(!r.is_empty());
        let raw = ":100644 100644 e f M\0outro/a.txt\0";
        let r = conferir_alcance(&versionada(), "phxclaw/", &analisar_raw(raw), "");
        assert!(r[0].contains("fora do projeto"), "{r:?}");
    }

    /// O caso verde: teste comum, e a calculadora com codigo puro, passam.
    #[test]
    fn teste_comum_e_calculadora_pura_passam() {
        let teste = "use phxclaw_agent::calculadora::avaliar;\nuse std::process::Command;\n\n\
mod comum;\n\n#[test]\nfn soma() {\n    assert_eq!(avaliar(\"1+1\").unwrap(), 2.0);\n    \
let _ = std::env::var(\"X\");\n    let _ = Command::new(\"true\");\n}\n";
        assert!(so(TESTE, teste).is_empty(), "{:?}", so(TESTE, teste));
        assert!(so("crates/phxclaw-office/tests/dados/a.json", "{}\n").is_empty());
        let calc = "fn hipot(a: f64, b: f64) -> f64 {\n    f64::sqrt(a * a + b * b)\n}\n\
fn nomes() -> Vec<String> {\n    let v: std::collections::BTreeMap<u8, u8> = Default::default();\n    \
v.keys().map(|k| k.to_string()).collect::<Vec<_>>()\n}\nfn f(x: &Value) -> Option<&str> {\n    \
Value::as_str(x)\n}\n#[cfg(test)]\nmod tests {\n    #[test]\n    fn t() {}\n}\n";
        assert!(so(CALC, calc).is_empty(), "{:?}", so(CALC, calc));
    }

    /// B3: o registro volta do disco sem garantia. Ramo adulterado e recusado ANTES de
    /// qualquer git (o projeto aqui nem e repositorio: o erro e o da forma, nao o do git), e o
    /// comando sai com aspas mesmo assim.
    #[test]
    fn registro_adulterado_e_recusado_e_o_comando_sai_com_aspas() {
        let d = std::env::temp_dir().join(format!("phx-evo-reg-{}", phxclaw_types::new_uuid_v7()));
        std::fs::create_dir_all(&d).unwrap();
        let mut r = registro("calculadora-20261009-120000");
        r.ramo = "x; rm -rf /".into();
        let e = conferir_registro(&d, &r.id.clone(), &r).unwrap_err();
        assert!(e.contains("adulterado") && e.contains("ramo"), "{e}");
        let cmd = comando_de_merge(&r);
        assert_eq!(cmd, "git -C '/x' merge --no-ff 'x; rm -rf /'");
        r.ramo = "it's".into();
        assert_eq!(aspas_de_shell(&r.ramo), r"'it'\''s'");
        for (campo, valor) in [
            ("id", "calculadora-20261009-120001"),
            ("item", "calc;x"),
            ("ramo", "evolucao/calculadora-20261009-120000/../x"),
            ("ramo", "evolucao/outro-20261009-120000"),
            ("commit", "HEAD; rm"),
        ] {
            let mut r = registro("calculadora-20261009-120000");
            match campo {
                "id" => r.id = valor.into(),
                "item" => r.item = valor.into(),
                "ramo" => r.ramo = valor.into(),
                _ => r.commit = Some(valor.into()),
            }
            let e = conferir_registro(&d, "calculadora-20261009-120000", &r).unwrap_err();
            assert!(e.contains("adulterado"), "{campo}={valor}: {e}");
        }
        let _ = std::fs::remove_dir_all(&d);
    }

    fn registro(id: &str) -> Registro {
        Registro {
            id: id.into(),
            item: "calculadora".into(),
            porque: String::new(),
            criado_em: Utc::now(),
            modelo: "m".into(),
            nota_do_modelo: None,
            repositorio: "/x".into(),
            base: "b".into(),
            ramo: format!("{PREFIXO_DO_RAMO}{id}"),
            commit: Some("a".repeat(40)),
            estado: Estado::EsperandoGo,
            etapa: None,
            motivo: None,
            plano: vec![],
            tarefa: None,
            arquivos: vec![],
            portoes: vec![],
            revisao: None,
            decidido_em: None,
        }
    }

    #[test]
    fn escolha_pula_vetado_e_proposta_aberta_e_prefere_parcial() {
        let d = std::env::temp_dir().join(format!("phx-evo-esc-{}", phxclaw_types::new_uuid_v7()));
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(
            d.join("b.json"),
            r#"{"estados":{"a_nao":{"estado":"nao"},"b_parcial":{"estado":"parcial"},
"c_feito":{"estado":"agente"},"d_vetado":{"estado":"parcial"},"e_parcial":{"estado":"parcial"}}}"#,
        )
        .unwrap();
        let mut p = pol(&[]);
        p.itens_vetados.push(ItemVetado {
            item: "d_vetado".into(),
            motivo: "seguranca".into(),
        });
        let (i, porque) = escolher(&d, &p, None).unwrap();
        assert_eq!(i.nome, "b_parcial");
        assert!(porque.contains("parcial"), "{porque}");
        assert!(
            escolher(&d, &p, Some("d_vetado"))
                .unwrap_err()
                .contains("seguranca")
        );
        assert!(escolher(&d, &p, Some("zz")).is_err());
        // proposta aberta para b_parcial: o proximo e e_parcial, e o --item recusa
        let r = Registro {
            id: "b_parcial-1".into(),
            item: "b_parcial".into(),
            porque: String::new(),
            criado_em: Utc::now(),
            modelo: "m".into(),
            nota_do_modelo: None,
            repositorio: "/x".into(),
            base: "b".into(),
            ramo: "evolucao/b_parcial-1".into(),
            commit: Some("c".into()),
            estado: Estado::EsperandoGo,
            etapa: None,
            motivo: None,
            plano: vec![],
            tarefa: None,
            arquivos: vec![],
            portoes: vec![],
            revisao: None,
            decidido_em: None,
        };
        gravar(&d, &r).unwrap();
        assert_eq!(escolher(&d, &p, None).unwrap().0.nome, "e_parcial");
        assert!(escolher(&d, &p, Some("b_parcial")).is_err());
        let _ = std::fs::remove_dir_all(&d);
    }

    /// O git do hospedeiro so roda os tres verbos do produto: outro verbo (ou uma opcao
    /// global na frente dele, como `-c core.fsmonitor=...`) cai antes de criar processo.
    #[test]
    fn git_do_hospedeiro_recusa_verbo_fora_da_lista() {
        for args in [
            &["status"][..],
            &["-c", "core.fsmonitor=/bin/true", "rev-parse", "HEAD"][..],
            &["config", "core.hooksPath"][..],
            &[][..],
        ] {
            let e = git_hospedeiro(Path::new("/"), args).unwrap_err();
            assert!(e.contains("recusado no hospedeiro"), "{args:?}: {e}");
        }
    }

    #[test]
    fn id_da_cli_nao_vira_caminho() {
        for ruim in ["", "../x", ".a", "-a", "a/b", "a b"] {
            assert!(validar_id(ruim).is_err(), "{ruim:?}");
        }
        assert!(validar_id("item_x-20261009-120000").is_ok());
    }
}
