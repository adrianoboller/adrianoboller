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
//! - **O alcance e conferido no diff, em codigo, e nao no prompt.** O objetivo diz ao modelo
//!   o que nao tocar, mas quem recusa e `conferir_alcance`, contra a lista versionada em
//!   `config/evolucao-politica.json`. Prompt e pedido; o portao e a regra. Politica que nao
//!   veta a si mesma nem este modulo e recusada inteira: sem isso, um ciclo poderia propor o
//!   afrouxamento da propria cerca, e o Go humano seria a unica trava.
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
/// O que a politica tem de vetar para ser aceita: ela mesma e o motor que a le.
pub const VETO_OBRIGATORIO: &[&str] = &[POLITICA, "crates/phxclaw-agent/src/evolucao.rs"];

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
        if let Some(v) = self
            .vetados
            .iter()
            .find(|v| v.trim().is_empty() || v.starts_with('/') || v.split('/').any(|s| s == ".."))
        {
            return Err(format!("caminho vetado invalido: {v:?}"));
        }
        for obrigatorio in VETO_OBRIGATORIO {
            if self.veto(obrigatorio).is_none() {
                return Err(format!(
                    "a politica nao veta {obrigatorio}: a auto-evolucao nao pode propor a propria cerca"
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

/// Os motivos de recusa do diff; vazio = dentro do alcance. `prefixo` e a pasta do projeto
/// dentro do repositorio (`phxclaw/`), como o `git rev-parse --show-prefix` a da.
pub fn conferir_alcance(pol: &Politica, prefixo: &str, mudancas: &[Mudanca]) -> Vec<String> {
    let mut recusas = Vec::new();
    for m in mudancas {
        let Some(rel) = m.caminho.strip_prefix(prefixo) else {
            recusas.push(format!("{}: fora do projeto ({prefixo})", m.caminho));
            continue;
        };
        if let Some(p) = pol.veto(rel) {
            recusas.push(format!("{rel}: caminho vetado ({p})"));
            continue;
        }
        // Link e submodulo: o caminho que o diff mostra nao e o que eles alcancam.
        match m.modo_novo.as_str() {
            "120000" => recusas.push(format!("{rel}: link simbolico")),
            "160000" => recusas.push(format!("{rel}: submodulo")),
            _ => {}
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
        "alcance" => "o item so avancou por caminho vetado: so o humano decide se ele sai do \
alcance (itens_vetados)"
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
            "Rejeitado. O ramo ficou; apagar e do humano: `git -C {} branch -D {}`.\n",
            r.repositorio, r.ramo
        )),
        Estado::Vermelho | Estado::Recusado => t.push_str("Nada a decidir: o ramo nao nasceu.\n"),
    }
    t
}

pub fn comando_de_merge(r: &Registro) -> String {
    format!("git -C {} merge --no-ff {}", r.repositorio, r.ramo)
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
    mae.status = TaskStatus::Running;
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
    mae.status = if r.estado == Estado::EsperandoGo {
        TaskStatus::Completed
    } else {
        TaskStatus::Failed
    };
    mae.error = r.motivo.clone();
    mae.updated_at = Utc::now();
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
    // 1. Clone raso do commit do produto, no hospedeiro: o produto e confiavel e o destino
    // acabou de nascer. Dai em diante, so git no sandbox.
    if let Err(e) = std::fs::create_dir_all(m) {
        return Some(vermelho("clone", e.to_string()));
    }
    let clone = m.join("repo");
    let url = format!("file://{}", r.repositorio);
    if let Err(e) = git_hospedeiro(
        m,
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
    r.base = match git_hospedeiro(&clone, &["rev-parse", "HEAD"]) {
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
    let mudancas = analisar_raw(&raw);
    let recusas = conferir_alcance(pol, prefixo, &mudancas);
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
Nao toque nestes caminhos do projeto (um portao recusa o diff inteiro): {}.\n\
Depois do fim rodam os portoes {} e uma revisao do diff; so com tudo verde o ramo nasce, e ele \
espera o Go humano.",
        item.nome,
        if item.evidencia.is_empty() {
            "(sem evidencia registrada)"
        } else {
            &item.evidencia
        },
        pol.vetados.join(", "),
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

/// O git do hospedeiro, so no repositorio do produto (confiavel) ou no clone recem-nascido.
fn git_hospedeiro(dir: &Path, args: &[&str]) -> Result<String, String> {
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
            vetados: vetados.iter().map(|s| s.to_string()).collect(),
            itens_vetados: vec![],
            portoes: vec!["fmt".into()],
            revisao_bloqueia_em: "alta".into(),
        }
    }

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
    fn a_politica_versionada_veta_o_sandbox_e_a_si_mesma() {
        let p: Politica =
            serde_json::from_str(include_str!("../../../config/evolucao-politica.json")).unwrap();
        let p = p.validar().unwrap();
        for c in [
            "crates/phxclaw-sandbox/src/lib.rs",
            "crates/phxclaw-secret-broker/src/lib.rs",
            "crates/phxclaw-types/src/segredo.rs",
            "crates/phxclaw-agent/src/motor.rs",
            "crates/phxclaw-agent/src/rbac.rs",
            "crates/phxclaw-agent/src/montagem.rs",
            "crates/phxclaw-agent/src/evolucao.rs",
            "config/evolucao-politica.json",
            ".github/workflows/ci.yml",
            "deploy/Dockerfile",
        ] {
            assert!(p.veto(c).is_some(), "{c} deveria estar vetado");
        }
        assert!(p.veto("crates/phxclaw-agent/src/calculadora.rs").is_none());
        assert!(
            p.veto("crates/phxclaw-agent/tests/calculadora.rs")
                .is_none()
        );
    }

    #[test]
    fn politica_que_nao_veta_a_si_mesma_e_recusada() {
        let e = pol(&["crates/phxclaw-sandbox/"]).validar().unwrap_err();
        assert!(e.contains(POLITICA), "{e}");
        let ok = pol(&[POLITICA, "crates/phxclaw-agent/src/evolucao.rs"]);
        assert!(ok.clone().validar().is_ok());
        let mut sem_portao = ok.clone();
        sem_portao.portoes.clear();
        assert!(sem_portao.validar().is_err());
        let mut fuga = ok;
        fuga.vetados.push("../fora".into());
        assert!(fuga.validar().is_err());
    }

    #[test]
    fn alcance_recusa_vetado_fora_do_projeto_e_link() {
        let p = pol(&["crates/phxclaw-sandbox/"]);
        let raw = ":100644 100644 a b M\0phxclaw/crates/phxclaw-sandbox/src/lib.rs\0\
:000000 100644 0 c A\0phxclaw/crates/x/tests/t.rs\0\
:000000 120000 0 d A\0phxclaw/crates/x/link\0\
:100644 100644 e f M\0outro/a.txt\0";
        let m = analisar_raw(raw);
        assert_eq!(m.len(), 4, "{m:?}");
        assert_eq!(m[1].estado, 'A');
        let r = conferir_alcance(&p, "phxclaw/", &m);
        assert_eq!(r.len(), 3, "{r:?}");
        assert!(r[0].contains("caminho vetado"), "{r:?}");
        assert!(r[1].contains("link simbolico"), "{r:?}");
        assert!(r[2].contains("fora do projeto"), "{r:?}");
        assert!(conferir_alcance(&p, "phxclaw/", &m[1..2]).is_empty());
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

    #[test]
    fn id_da_cli_nao_vira_caminho() {
        for ruim in ["", "../x", ".a", "-a", "a/b", "a b"] {
            assert!(validar_id(ruim).is_err(), "{ruim:?}");
        }
        assert!(validar_id("item_x-20261009-120000").is_ok());
    }
}
