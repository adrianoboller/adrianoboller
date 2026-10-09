//! `git`, `git_write` e `git_worktree`: o binario git de verdade, no MESMO bwrap do `shell`,
//! com saida estruturada (arquivos mudados, hunks, commits, linhas do blame).
//!
//! Decisoes que valem saber antes de mexer:
//!
//! - **Um sandbox so.** O git roda por `run_in_workdir_com`, a funcao do `shell`, com o que
//!   ele precisa a mais entrando pelo `SandboxExtras` (so ambiente: o `/usr` ja esta
//!   montado). Uma segunda lista de binds divergiria da primeira no dia em que alguem
//!   endurecesse so uma.
//! - **O modelo nunca escreve a linha de comando.** Cada acao monta o argv dela; o que vem
//!   do modelo entra como argumento entre aspas simples, referencia comecando com `-` e
//!   recusada (seria opcao), e caminho vai depois de `--`.
//! - **O git nao executa codigo que o repositorio declara.** Tudo no motor (`rodar_git`),
//!   nunca acao por acao: ganchos para `/dev/null`, `fsmonitor` desligado, assinatura nao
//!   conferida nem pedida (`log.showSignature=false`, `gpg.*.program=/bin/false`,
//!   `commit.gpgSign=false`), e antes de cada chamada o `.git/config` e LIDO (`git config
//!   --name-only`, que nao executa nada) e todo `filter.*.clean/smudge/process`,
//!   `diff.*.textconv/command` e `merge.*.driver` declarado vira vazio por `-c`. Chave com
//!   `=` no nome nao se neutraliza por `-c`, entao a chamada e recusada. `--no-textconv` e
//!   `--no-ext-diff` continuam nas acoes que mostram conteudo, como segunda trava.
//!   O que sobra, dito para nao se prometer mais que isso: `include.path` aponta so para
//!   arquivo de config (lido, nao executado), e qualquer comando continua preso no sandbox
//!   sem rede em /work, que e o poder do `shell` ja concedido. Medido em 01/10/2026 contra
//!   o git 2.43 no bwrap: log de commit assinado, blame com textconv e status com filtro
//!   `clean` rodavam o script do repositorio; com o motor, nenhum roda.
//! - **Um motor de diff.** `diff_do_repo` e `analisar_diff` servem o `git diff`, o
//!   `code_review` e o `phxclaw revisar`; o diff de PR do GitHub/GitLab passa pelo mesmo
//!   analisador.

use crate::python::aspas;
use crate::sistema::caminho_do_projeto;
use phxclaw_agent_core::{BoxFut, Tool, ToolContext, ToolError, ToolOutput, ToolSpec};
use phxclaw_sandbox::{SandboxExtras, WorkdirCommand, WorkdirOutput, run_in_workdir_com};
use serde::Serialize;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Teto do diff devolvido ao modelo; acima dele o diff corta em fronteira de arquivo e diz
/// que cortou.
pub const DIFF_MAX_BYTES: usize = 256 * 1024;

/// Configuracoes passadas por `-c` em TODA chamada: valem sobre a do repositorio.
const CONFIG_SEGURA: &[&str] = &[
    "core.hooksPath=/dev/null",
    "core.fsmonitor=false",
    "core.pager=cat",
    "color.ui=false",
    "core.quotepath=false",
    "protocol.allow=never",
    "safe.directory=*",
    "advice.detachedHead=false",
    // Conferir assinatura executa `gpg.program`, que o repositorio escolhe.
    "log.showSignature=false",
    "gpg.program=/bin/false",
    "gpg.ssh.program=/bin/false",
    "gpg.x509.program=/bin/false",
    "commit.gpgSign=false",
    "tag.gpgSign=false",
    "core.sshCommand=/bin/false",
    "credential.helper=",
];

/// Prefixo de shell que le as chaves que executam programa e as zera por `-c` (em `$@`).
/// Le com a MESMA configuracao segura da chamada: sem o `safe.directory=*` daqui, a leitura
/// falharia num repositorio de outro dono e a chamada seguiria sem neutralizar nada.
fn neutralizar(seguras: &str, repo_c: &str) -> String {
    format!(
        "set --\n\
while IFS= read -r k; do\n\
  case \"$k\" in\n\
    '') ;;\n\
    *=*) echo \"configuracao que o agente nao consegue neutralizar: $k\" >&2; exit 97 ;;\n\
    filter.*.clean|filter.*.smudge|filter.*.process|diff.*.textconv|diff.*.command|merge.*.driver) set -- \"$@\" -c \"$k=\" ;;\n\
  esac\n\
done <<FIM\n\
$(git{seguras}{repo_c} config --name-only --get-regexp '^(filter|diff|merge)\\.' 2>/dev/null)\n\
FIM\n"
    )
}

/// Separadores do `--format`: unidade e registro, que nao aparecem em mensagem de commit.
const US: char = '\u{1f}';
const RS: char = '\u{1e}';
const FORMATO_COMMIT: &str = "%H%x1f%h%x1f%an%x1f%ae%x1f%aI%x1f%P%x1f%s%x1f%b%x1e";

/// Roda `git <args>` na pasta `repo` (relativa a /work) dentro do sandbox do shell.
pub async fn rodar_git(
    bwrap: &Path,
    workdir: &Path,
    repo: &str,
    args: Vec<String>,
    timeout: Duration,
) -> Result<WorkdirOutput, ToolError> {
    let args: String = args.iter().map(|a| format!(" {}", aspas(a))).collect();
    rodar_roteiro_git(
        bwrap,
        workdir,
        repo,
        |git| format!("exec {git}{args}"),
        timeout,
        4 * 1024 * 1024,
    )
    .await
}

/// O mesmo motor do `rodar_git` para quem precisa de mais de um git na mesma chamada (a
/// varredura de segredos monta um indice temporario e le o diff dele). `corpo` recebe o
/// prefixo `git -c ... "$@" -C repo` ja neutralizado e devolve as linhas de shell: assim a
/// configuracao segura e a neutralizacao continuam escritas uma vez so.
pub async fn rodar_roteiro_git(
    bwrap: &Path,
    workdir: &Path,
    repo: &str,
    corpo: impl FnOnce(&str) -> String,
    timeout: Duration,
    max_output_bytes: usize,
) -> Result<WorkdirOutput, ToolError> {
    let seguras: String = CONFIG_SEGURA
        .iter()
        .map(|c| format!(" -c {}", aspas(c)))
        .collect();
    let repo_c = if repo.is_empty() {
        String::new()
    } else {
        format!(" -C {}", aspas(repo))
    };
    let mut script = neutralizar(&seguras, &repo_c);
    script.push_str(&corpo(&format!("git{seguras} \"$@\"{repo_c}")));
    let cmd = WorkdirCommand {
        workdir: workdir.to_path_buf(),
        script,
        timeout,
        network: false,
        max_output_bytes,
    };
    let extras = SandboxExtras {
        ro_binds: vec![],
        env: [
            ("GIT_CONFIG_NOSYSTEM", "1"),
            ("GIT_CONFIG_GLOBAL", "/dev/null"),
            ("GIT_TERMINAL_PROMPT", "0"),
            ("GIT_AUTHOR_NAME", "PhxClaw agente"),
            ("GIT_AUTHOR_EMAIL", "agente@phxclaw.local"),
            ("GIT_COMMITTER_NAME", "PhxClaw agente"),
            ("GIT_COMMITTER_EMAIL", "agente@phxclaw.local"),
            ("LC_ALL", "C"),
            // `status` sem atualizar o indice: leitura que nao grava nada no repositorio.
            ("GIT_OPTIONAL_LOCKS", "0"),
            ("GIT_EDITOR", "true"),
        ]
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect(),
    };
    let bwrap = bwrap.to_path_buf();
    tokio::task::spawn_blocking(move || run_in_workdir_com(&bwrap, &cmd, &extras))
        .await
        .map_err(|e| ToolError::Failed(e.to_string()))?
        .map_err(|e| ToolError::Failed(e.to_string()))
}

/// Saida de sucesso, ou erro com o que o git disse.
fn exigir_sucesso(acao: &str, s: WorkdirOutput) -> Result<String, ToolError> {
    if s.exit_code == Some(0) {
        return Ok(s.stdout);
    }
    let msg = if s.stderr.trim().is_empty() {
        s.stdout
    } else {
        s.stderr
    };
    Err(ToolError::Failed(format!(
        "git {acao} falhou (saida {}): {}",
        s.exit_code
            .map(|c| c.to_string())
            .unwrap_or_else(|| "sinal".into()),
        msg.trim()
    )))
}

/// Referencia (branch, commit, `HEAD~2`, `main..topico`) vinda do modelo. Comecar com `-`
/// faria dela uma opcao do git: `--output=/work/x` grava arquivo a partir de um `diff`.
pub fn referencia(s: &str) -> Result<String, ToolError> {
    let s = s.trim();
    let ok = !s.is_empty()
        && !s.starts_with('-')
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || "._/@^~{}:+-".contains(c));
    if !ok {
        return Err(ToolError::InvalidArguments(format!(
            "referencia invalida: {s:?} (nao pode comecar com '-'; letras, digitos e ._/@^~{{}}:+-)"
        )));
    }
    Ok(s.to_string())
}

/// Nome de ramo, ou sha, para as acoes que CRIAM ou movem ramo (`init`, `checkout`,
/// `branch_*`, `git_worktree`). Sem `@{`, `^`, `~`, `:` nem `..`: com eles, `stash@{0}^3`
/// (o commit dos nao rastreados de um stash) virava ramo, e o que entra num ramo tem de ter
/// passado pela varredura de segredos. As acoes que so LEEM continuam com `referencia`.
pub fn nome_de_ramo(s: &str) -> Result<String, ToolError> {
    let r = referencia(s)?;
    let ok = !r.starts_with('.')
        && !r.contains("..")
        && !r.ends_with(".lock")
        && r.chars()
            .all(|c| c.is_ascii_alphanumeric() || "._/-".contains(c));
    if !ok {
        return Err(ToolError::InvalidArguments(format!(
            "nome de ramo invalido: {r:?} (so nome de ramo ou sha: letras, digitos e ._/-; sem \
             @{{, ^, ~, : ou ..)"
        )));
    }
    Ok(r)
}

fn lista_de_caminhos(args: &Value) -> Vec<String> {
    match args.get("paths") {
        Some(Value::Array(a)) => a
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_string)
            .collect(),
        Some(Value::String(s)) if !s.trim().is_empty() => vec![s.clone()],
        _ => vec![],
    }
}

fn texto<'a>(args: &'a Value, nome: &str) -> Option<&'a str> {
    args.get(nome)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
}

fn exigir<'a>(args: &'a Value, nome: &str) -> Result<&'a str, ToolError> {
    texto(args, nome)
        .ok_or_else(|| ToolError::InvalidArguments(format!("falta o campo texto '{nome}'")))
}

fn booleano(args: &Value, nome: &str) -> bool {
    args.get(nome).and_then(Value::as_bool).unwrap_or(false)
}

fn inteiro(args: &Value, nome: &str, padrao: u64, max: u64) -> u64 {
    args.get(nome)
        .and_then(|v| v.as_u64().or_else(|| v.as_str()?.trim().parse().ok()))
        .unwrap_or(padrao)
        .clamp(1, max)
}

// ---------------------------------------------------------------- diff

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Hunk {
    pub antigo_inicio: u64,
    pub antigo_linhas: u64,
    pub novo_inicio: u64,
    pub novo_linhas: u64,
    /// O que o git escreve depois do segundo `@@` (a funcao onde o hunk cai).
    pub contexto: String,
    pub linhas: Vec<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ArquivoDiff {
    /// Caminho no lado novo; no arquivo removido, o do lado antigo.
    pub caminho: String,
    pub antigo: Option<String>,
    /// adicionado, removido, renomeado, modificado.
    pub estado: String,
    pub binario: bool,
    pub adicoes: u64,
    pub remocoes: u64,
    pub hunks: Vec<Hunk>,
}

impl ArquivoDiff {
    /// A linha `n` do arquivo novo esta dentro de algum hunk? E a pergunta que o
    /// `code_review` faz para recusar achado em linha que o diff nem mostra.
    pub fn linha_no_diff(&self, n: u64) -> bool {
        self.hunks
            .iter()
            .any(|h| n >= h.novo_inicio && n < h.novo_inicio + h.novo_linhas.max(1))
    }
}

fn sem_prefixo(p: &str) -> String {
    let p = p.trim_end_matches('\t').trim();
    let p = p.trim_matches('"');
    p.strip_prefix("a/")
        .or_else(|| p.strip_prefix("b/"))
        .unwrap_or(p)
        .to_string()
}

/// `@@ -a,b +c,d @@ contexto`; contagem omitida vale 1.
fn cabecalho_de_hunk(l: &str) -> Option<Hunk> {
    let resto = l.strip_prefix("@@ ")?;
    let (faixas, contexto) = resto.split_once(" @@")?;
    let mut partes = faixas.split(' ');
    let faixa = |s: Option<&str>, sinal: char| -> Option<(u64, u64)> {
        let s = s?.strip_prefix(sinal)?;
        Some(match s.split_once(',') {
            Some((a, b)) => (a.parse().ok()?, b.parse().ok()?),
            None => (s.parse().ok()?, 1),
        })
    };
    let (ai, al) = faixa(partes.next(), '-')?;
    let (ni, nl) = faixa(partes.next(), '+')?;
    Some(Hunk {
        antigo_inicio: ai,
        antigo_linhas: al,
        novo_inicio: ni,
        novo_linhas: nl,
        contexto: contexto.trim().to_string(),
        linhas: vec![],
    })
}

/// Diff unificado (do git, do GitHub ou montado do GitLab) em arquivos e hunks.
pub fn analisar_diff(texto: &str) -> Vec<ArquivoDiff> {
    let mut v: Vec<ArquivoDiff> = Vec::new();
    let novo_arquivo = |caminho: String| ArquivoDiff {
        caminho,
        antigo: None,
        estado: "modificado".into(),
        binario: false,
        adicoes: 0,
        remocoes: 0,
        hunks: vec![],
    };
    // Dentro de um hunk, linhas que comecam com `---`/`+++` sao conteudo, nao cabecalho:
    // so o que falta do hunk decide.
    let (mut falta_antigo, mut falta_novo) = (0u64, 0u64);
    for l in texto.lines() {
        if falta_antigo > 0 || falta_novo > 0 {
            if let Some(a) = v.last_mut()
                && let Some(h) = a.hunks.last_mut()
            {
                match l.chars().next() {
                    Some('+') => {
                        a.adicoes += 1;
                        falta_novo = falta_novo.saturating_sub(1);
                    }
                    Some('-') => {
                        a.remocoes += 1;
                        falta_antigo = falta_antigo.saturating_sub(1);
                    }
                    Some('\\') => {}
                    _ => {
                        falta_novo = falta_novo.saturating_sub(1);
                        falta_antigo = falta_antigo.saturating_sub(1);
                    }
                }
                h.linhas.push(l.to_string());
                continue;
            }
            falta_antigo = 0;
            falta_novo = 0;
        }
        if let Some(r) = l.strip_prefix("diff --git ") {
            // `a/x b/x`: o caminho certo vem do ---/+++ ou do rename; isto e o palpite.
            let caminho = r
                .rsplit_once(" b/")
                .map(|(_, b)| b.to_string())
                .unwrap_or_else(|| sem_prefixo(r));
            v.push(novo_arquivo(caminho));
        } else if let Some(antigo) = l.strip_prefix("--- ") {
            // Patch cru (sem `diff --git`): o `---` abre o arquivo seguinte.
            if v.last().is_none_or(|a| !a.hunks.is_empty()) {
                v.push(novo_arquivo(sem_prefixo(antigo)));
            }
            let a = v.last_mut().unwrap();
            if antigo.trim() == "/dev/null" {
                a.estado = "adicionado".into();
            } else {
                a.antigo = Some(sem_prefixo(antigo));
            }
        } else if let Some(n) = l.strip_prefix("+++ ") {
            if let Some(a) = v.last_mut() {
                if n.trim() == "/dev/null" {
                    a.estado = "removido".into();
                    if let Some(an) = &a.antigo {
                        a.caminho = an.clone();
                    }
                } else {
                    a.caminho = sem_prefixo(n);
                }
            }
        } else if l.starts_with("new file mode") {
            if let Some(a) = v.last_mut() {
                a.estado = "adicionado".into();
            }
        } else if l.starts_with("deleted file mode") {
            if let Some(a) = v.last_mut() {
                a.estado = "removido".into();
            }
        } else if let Some(de) = l.strip_prefix("rename from ") {
            if let Some(a) = v.last_mut() {
                a.estado = "renomeado".into();
                a.antigo = Some(de.to_string());
            }
        } else if let Some(para) = l.strip_prefix("rename to ") {
            if let Some(a) = v.last_mut() {
                a.caminho = para.to_string();
            }
        } else if l.starts_with("Binary files ") || l.starts_with("GIT binary patch") {
            if let Some(a) = v.last_mut() {
                a.binario = true;
            }
        } else if let Some(h) = cabecalho_de_hunk(l) {
            if v.is_empty() {
                v.push(novo_arquivo(String::new()));
            }
            falta_antigo = h.antigo_linhas;
            falta_novo = h.novo_linhas;
            v.last_mut().unwrap().hunks.push(h);
        }
    }
    // Sem `---` o caminho antigo so existe no rename; no resto e o mesmo.
    for a in &mut v {
        if a.estado == "modificado" && a.antigo.as_deref() == Some(a.caminho.as_str()) {
            a.antigo = None;
        }
        if a.estado == "removido" {
            a.antigo = None;
        }
    }
    v
}

// ---------------------------------------------------------------- trecho (stage parcial)

/// Teto do patch de um trecho: ele viaja dentro da linha de shell do sandbox (heredoc), e
/// um argumento do `sh -c` nao passa de 128 KiB no Linux.
pub const TRECHO_MAX_BYTES: usize = 64 * 1024;

/// O que vai ao indice de um arquivo: hunks inteiros (pelo indice do `git diff`) ou uma
/// faixa de linhas, na numeracao do arquivo NOVO.
#[derive(Debug, Clone, PartialEq)]
pub enum Trecho {
    Hunks(Vec<usize>),
    Linhas(u64, u64),
}

/// Monta o patch que leva so o trecho ao indice, a partir do diff do arquivo (arvore
/// contra indice). Na faixa de linhas, dentro de um hunk: `+` fora da faixa cai, `-` fora
/// da faixa vira contexto -- e o que o `git add -p` com `e` faz a mao. O inicio do lado
/// novo de cada hunk e recalculado, porque hunk pulado desloca os seguintes.
pub fn patch_do_trecho(arq: &ArquivoDiff, trecho: &Trecho) -> Result<String, ToolError> {
    if arq.binario {
        return Err(ToolError::InvalidArguments(format!(
            "{}: arquivo binario nao se prepara por trecho",
            arq.caminho
        )));
    }
    if let Trecho::Hunks(idx) = trecho
        && let Some(i) = idx.iter().find(|&&i| i >= arq.hunks.len())
    {
        return Err(ToolError::InvalidArguments(format!(
            "{}: trecho {i} nao existe (o diff tem {} trecho(s), de 0 a {})",
            arq.caminho,
            arq.hunks.len(),
            arq.hunks.len().saturating_sub(1)
        )));
    }
    let mut corpo = String::new();
    let mut delta: i64 = 0;
    for (i, h) in arq.hunks.iter().enumerate() {
        let linhas: Vec<String> = match trecho {
            Trecho::Hunks(idx) => {
                if !idx.contains(&i) {
                    continue;
                }
                h.linhas.clone()
            }
            Trecho::Linhas(ini, fim) => {
                let mut novo = h.novo_inicio;
                let mut saida = Vec::new();
                // `\ No newline at end of file` pertence a linha anterior: segue o destino dela.
                let mut anterior_ficou = true;
                for l in &h.linhas {
                    match l.chars().next() {
                        Some('+') => {
                            let dentro = novo >= *ini && novo <= *fim;
                            novo += 1;
                            anterior_ficou = dentro;
                            if dentro {
                                saida.push(l.clone());
                            }
                        }
                        Some('-') => {
                            // A linha apagada «mora» onde ela estaria no arquivo novo.
                            anterior_ficou = true;
                            if novo >= *ini && novo <= *fim {
                                saida.push(l.clone());
                            } else {
                                saida.push(format!(" {}", &l[1..]));
                            }
                        }
                        Some('\\') => {
                            if anterior_ficou {
                                saida.push(l.clone());
                            }
                        }
                        _ => {
                            novo += 1;
                            anterior_ficou = true;
                            saida.push(l.clone());
                        }
                    }
                }
                saida
            }
        };
        let al = linhas
            .iter()
            .filter(|l| !l.starts_with('+') && !l.starts_with('\\'))
            .count() as u64;
        let nl = linhas
            .iter()
            .filter(|l| !l.starts_with('-') && !l.starts_with('\\'))
            .count() as u64;
        if linhas
            .iter()
            .all(|l| !l.starts_with('+') && !l.starts_with('-'))
        {
            continue;
        }
        let ni = (h.antigo_inicio as i64 + delta).max(0) as u64;
        delta += nl as i64 - al as i64;
        corpo.push_str(&format!("@@ -{},{al} +{ni},{nl} @@\n", h.antigo_inicio));
        for l in &linhas {
            corpo.push_str(l);
            corpo.push('\n');
        }
    }
    if corpo.is_empty() {
        return Err(ToolError::InvalidArguments(format!(
            "{}: nenhuma mudanca no trecho pedido",
            arq.caminho
        )));
    }
    let c = &arq.caminho;
    let patch = format!("diff --git a/{c} b/{c}\n--- a/{c}\n+++ b/{c}\n{corpo}");
    if patch.len() > TRECHO_MAX_BYTES {
        return Err(ToolError::InvalidArguments(format!(
            "o trecho passa de {} KiB; prepare em partes menores",
            TRECHO_MAX_BYTES / 1024
        )));
    }
    Ok(patch)
}

// ---------------------------------------------------------------- conflitos de merge

/// Um bloco `<<<<<<<`/`=======`/`>>>>>>>` de um arquivo em conflito, com as duas versoes
/// (e a base, quando o `merge.conflictStyle` e diff3).
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Bloco {
    pub indice: usize,
    /// Linha (1 em diante) do `<<<<<<<` e do `>>>>>>>`.
    pub de: usize,
    pub ate: usize,
    pub rotulo_nosso: String,
    pub rotulo_deles: String,
    pub nosso: Vec<String>,
    pub deles: Vec<String>,
    pub base: Option<Vec<String>>,
}

/// Os blocos de conflito de um texto. Marcador fora de ordem (um `=======` sem `<<<<<<<`,
/// um `<<<<<<<` sem fim) nao e bloco: o texto fica como esta e o chamador ve zero blocos
/// ali, em vez de uma resolucao que apagaria linhas que nao eram conflito.
pub fn blocos_de_conflito(texto: &str) -> Vec<Bloco> {
    let mut v = Vec::new();
    let linhas: Vec<&str> = texto.lines().collect();
    let mut i = 0;
    while i < linhas.len() {
        let Some(rn) = linhas[i].strip_prefix("<<<<<<< ") else {
            i += 1;
            continue;
        };
        let (mut nosso, mut base, mut deles) = (Vec::new(), None::<Vec<String>>, Vec::new());
        let mut fase = 0u8;
        let mut fim = None;
        for (j, l) in linhas.iter().enumerate().skip(i + 1) {
            if fase < 2 && l.starts_with("||||||| ") {
                fase = 1;
                base = Some(Vec::new());
            } else if fase < 2 && *l == "=======" {
                fase = 2;
            } else if fase == 2
                && let Some(rd) = l.strip_prefix(">>>>>>> ")
            {
                v.push(Bloco {
                    indice: v.len(),
                    de: i + 1,
                    ate: j + 1,
                    rotulo_nosso: rn.to_string(),
                    rotulo_deles: rd.to_string(),
                    nosso: std::mem::take(&mut nosso),
                    deles: std::mem::take(&mut deles),
                    base: base.take(),
                });
                fim = Some(j);
                break;
            } else if l.starts_with("<<<<<<< ") {
                break;
            } else {
                match fase {
                    0 => nosso.push(l.to_string()),
                    1 => base.get_or_insert_with(Vec::new).push(l.to_string()),
                    _ => deles.push(l.to_string()),
                }
            }
        }
        i = fim.map(|f| f + 1).unwrap_or(i + 1);
    }
    v
}

/// O texto com o bloco `indice` trocado por `escolha`. Devolve o texto novo e quantos
/// blocos sobraram.
pub fn resolver_bloco(
    texto: &str,
    indice: usize,
    escolha: &[String],
) -> Result<(String, usize), ToolError> {
    let blocos = blocos_de_conflito(texto);
    let Some(b) = blocos.iter().find(|b| b.indice == indice) else {
        return Err(ToolError::InvalidArguments(format!(
            "bloco {indice} nao existe (o arquivo tem {} bloco(s) de conflito)",
            blocos.len()
        )));
    };
    let linhas: Vec<&str> = texto.lines().collect();
    let mut saida: Vec<&str> = linhas[..b.de - 1].to_vec();
    saida.extend(escolha.iter().map(String::as_str));
    saida.extend(linhas[b.ate..].iter().copied());
    let mut novo = saida.join("\n");
    if texto.ends_with('\n') || texto.is_empty() {
        novo.push('\n');
    }
    let restantes = blocos_de_conflito(&novo).len();
    Ok((novo, restantes))
}

/// Corta o diff em fronteira de arquivo abaixo do teto. Devolve (diff, cortou).
pub fn limitar_diff(texto: &str, max: usize) -> (String, bool) {
    if texto.len() <= max {
        return (texto.to_string(), false);
    }
    // O ultimo `diff --git` que COMECA dentro do teto: o arquivo dele e o primeiro de fora.
    let corte = texto
        .match_indices("\ndiff --git ")
        .map(|(i, _)| i + 1)
        .take_while(|&i| i <= max)
        .last()
        .unwrap_or_else(|| {
            let mut c = max;
            while !texto.is_char_boundary(c) {
                c -= 1;
            }
            c
        });
    (texto[..corte].to_string(), true)
}

/// O diff de um repositorio, pelo git de verdade: o que o `git diff`, o `code_review` e o
/// `phxclaw revisar` leem. `rev` vazio compara a arvore com o indice (ou o indice com o
/// HEAD, se `cached`).
pub async fn diff_do_repo(
    bwrap: &Path,
    workdir: &Path,
    repo: &str,
    rev: Option<&str>,
    cached: bool,
    caminhos: &[String],
    timeout: Duration,
) -> Result<String, ToolError> {
    let mut a: Vec<String> = [
        "diff",
        "--no-color",
        "--no-ext-diff",
        "--no-textconv",
        "-M",
        "-U3",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    if cached {
        a.push("--cached".into());
    }
    if let Some(r) = rev {
        a.push(referencia(r)?);
    }
    a.push("--".into());
    a.extend(caminhos.iter().cloned());
    exigir_sucesso("diff", rodar_git(bwrap, workdir, repo, a, timeout).await?)
}

// ---------------------------------------------------------------- outros analisadores

/// `git status --porcelain=v1 -b -z`.
pub fn analisar_status(saida: &str) -> Value {
    let mut ramo = Value::Null;
    let mut upstream = Value::Null;
    let (mut ahead, mut behind) = (0u64, 0u64);
    let mut arquivos = Vec::new();
    let mut partes = saida.split('\0').filter(|s| !s.is_empty());
    while let Some(e) = partes.next() {
        if let Some(b) = e.strip_prefix("## ") {
            let (nomes, conta) = match b.split_once(" [") {
                Some((n, c)) => (n, c.trim_end_matches(']')),
                None => (b, ""),
            };
            let nomes = nomes
                .strip_prefix("No commits yet on ")
                .or_else(|| nomes.strip_prefix("Initial commit on "))
                .unwrap_or(nomes);
            match nomes.split_once("...") {
                Some((r, u)) => {
                    ramo = json!(r);
                    upstream = json!(u);
                }
                None => ramo = json!(nomes),
            }
            for c in conta.split(", ") {
                if let Some(n) = c.strip_prefix("ahead ") {
                    ahead = n.parse().unwrap_or(0);
                } else if let Some(n) = c.strip_prefix("behind ") {
                    behind = n.parse().unwrap_or(0);
                }
            }
            continue;
        }
        if e.len() < 4 {
            continue;
        }
        let (x, y, caminho) = (&e[0..1], &e[1..2], &e[3..]);
        // Rename/copia: o -z poe a origem como a entrada SEGUINTE.
        let de = if x == "R" || x == "C" || y == "R" || y == "C" {
            partes.next().map(str::to_string)
        } else {
            None
        };
        let estado = match (x, y) {
            ("?", "?") => "nao_rastreado",
            ("!", "!") => "ignorado",
            ("U", _) | (_, "U") | ("A", "A") | ("D", "D") => "conflito",
            ("R", _) | (_, "R") => "renomeado",
            ("A", _) => "adicionado",
            ("D", _) | (_, "D") => "removido",
            _ => "modificado",
        };
        arquivos.push(json!({
            "caminho": caminho,
            "indice": x,
            "arvore": y,
            "estado": estado,
            "preparado": x != " " && x != "?" && x != "!",
            "de": de,
        }));
    }
    json!({
        "ramo": ramo,
        "upstream": upstream,
        "a_frente": ahead,
        "atras": behind,
        "limpo": arquivos.is_empty(),
        "arquivos": arquivos,
    })
}

/// Registros do `FORMATO_COMMIT`.
pub fn analisar_commits(saida: &str) -> Vec<Value> {
    saida
        .split(RS)
        .map(|r| r.trim_start_matches('\n'))
        .filter(|r| !r.trim().is_empty())
        .filter_map(|r| {
            let c: Vec<&str> = r.split(US).collect();
            (c.len() >= 7).then(|| {
                json!({
                    "commit": c[0],
                    "curto": c[1],
                    "autor": c[2],
                    "email": c[3],
                    "data": c[4],
                    "pais": c[5].split_whitespace().collect::<Vec<_>>(),
                    "assunto": c[6],
                    "corpo": c.get(7).map(|b| b.trim()).unwrap_or(""),
                })
            })
        })
        .collect()
}

/// `git blame --porcelain`: o cabecalho do commit so vem na primeira linha dele.
pub fn analisar_blame(saida: &str) -> Vec<Value> {
    #[derive(Default, Clone)]
    struct Info {
        autor: String,
        data: String,
        resumo: String,
    }
    let mut infos: HashMap<String, Info> = HashMap::new();
    let mut linhas = Vec::new();
    let mut atual: Option<(String, u64)> = None;
    for l in saida.lines() {
        if let Some(t) = l.strip_prefix('\t') {
            if let Some((sha, n)) = atual.take() {
                let i = infos.get(&sha).cloned().unwrap_or_default();
                linhas.push(json!({
                    "linha": n,
                    "commit": sha.chars().take(12).collect::<String>(),
                    "autor": i.autor,
                    "data": i.data,
                    "resumo": i.resumo,
                    "texto": t,
                }));
            }
            continue;
        }
        let mut p = l.split(' ');
        let primeiro = p.next().unwrap_or("");
        if primeiro.len() == 40 && primeiro.chars().all(|c| c.is_ascii_hexdigit()) {
            let _orig = p.next();
            let fim: u64 = p.next().and_then(|s| s.parse().ok()).unwrap_or(0);
            infos.entry(primeiro.to_string()).or_default();
            atual = Some((primeiro.to_string(), fim));
            continue;
        }
        if let Some((sha, _)) = &atual {
            let i = infos.entry(sha.clone()).or_default();
            if let Some(a) = l.strip_prefix("author ") {
                i.autor = a.to_string();
            } else if let Some(t) = l.strip_prefix("author-time ") {
                i.data = t
                    .parse::<i64>()
                    .ok()
                    .and_then(|s| chrono::DateTime::from_timestamp(s, 0))
                    .map(|d| d.to_rfc3339())
                    .unwrap_or_default();
            } else if let Some(s) = l.strip_prefix("summary ") {
                i.resumo = s.to_string();
            }
        }
    }
    linhas
}

/// `git worktree list --porcelain`, com o caminho dito relativo a /work (e o que as outras
/// ferramentas aceitam como `path`).
pub fn analisar_worktrees(saida: &str) -> Vec<Value> {
    saida
        .split("\n\n")
        .filter(|b| !b.trim().is_empty())
        .map(|b| {
            let mut w = serde_json::Map::new();
            for l in b.lines() {
                let (k, v) = l.split_once(' ').unwrap_or((l, ""));
                match k {
                    "worktree" => {
                        let rel = v.strip_prefix("/work").unwrap_or(v).trim_start_matches('/');
                        w.insert("path".into(), json!(if rel.is_empty() { "." } else { rel }));
                    }
                    "HEAD" => {
                        w.insert("commit".into(), json!(v));
                    }
                    "branch" => {
                        w.insert(
                            "ramo".into(),
                            json!(v.strip_prefix("refs/heads/").unwrap_or(v)),
                        );
                    }
                    "detached" | "bare" | "locked" | "prunable" => {
                        w.insert(k.into(), json!(true));
                    }
                    _ => {}
                }
            }
            Value::Object(w)
        })
        .collect()
}

// ---------------------------------------------------------------- ferramentas

/// `git` (git.read) e `git_write` (git.write): a mesma ferramenta registrada duas vezes,
/// uma por capacidade, para o portao do motor continuar sendo o unico.
pub struct GitTool {
    pub bwrap: PathBuf,
    pub escrita: bool,
    pub timeout: Duration,
    /// Varredura de segredos antes de `add`/`commit` (`segredos.rs`).
    pub segredos: crate::segredos::Varredura,
}

impl GitTool {
    pub fn leitura(bwrap: PathBuf) -> Self {
        Self {
            bwrap,
            escrita: false,
            timeout: Duration::from_secs(60),
            segredos: crate::segredos::Varredura::default(),
        }
    }
    /// A configuracao da varredura e lida AQUI, e nao por quem monta: a nuvem
    /// (`parallel_tasks`) cria o seu `GitTool` de escrita, e uma varredura que dependesse
    /// de quem monta lembrar dela deixaria aquele caminho sem guarda.
    pub fn escrita(bwrap: PathBuf) -> Self {
        Self {
            bwrap,
            escrita: true,
            timeout: Duration::from_secs(60),
            segredos: crate::segredos::Varredura::da_configuracao(),
        }
    }

    /// A gravacao vai levar segredo? `Err(Denied)` bloqueia; `Ok` volta no resultado.
    async fn varrer(
        &self,
        ctx: &ToolContext,
        repo: &str,
        g: crate::segredos::Gravacao,
    ) -> Result<Value, ToolError> {
        let v = self
            .segredos
            .antes_de_gravar(
                &self.bwrap,
                &ctx.workdir,
                repo,
                &g,
                self.timeout.min(ctx.timeout),
            )
            .await?;
        Ok(v.json())
    }

    async fn git(
        &self,
        ctx: &ToolContext,
        repo: &str,
        args: Vec<String>,
    ) -> Result<String, ToolError> {
        let acao = args.first().cloned().unwrap_or_default();
        let s = rodar_git(
            &self.bwrap,
            &ctx.workdir,
            repo,
            args,
            self.timeout.min(ctx.timeout),
        )
        .await?;
        exigir_sucesso(&acao, s)
    }

    async fn ler(
        &self,
        ctx: &ToolContext,
        repo: &str,
        acao: &str,
        args: &Value,
    ) -> Result<Value, ToolError> {
        let s = |v: &[&str]| v.iter().map(|x| x.to_string()).collect::<Vec<_>>();
        match acao {
            "status" => {
                let o = self
                    .git(
                        ctx,
                        repo,
                        s(&[
                            "status",
                            "--porcelain=v1",
                            "-b",
                            "-z",
                            "--untracked-files=all",
                        ]),
                    )
                    .await?;
                Ok(analisar_status(&o))
            }
            "diff" => {
                let caminhos = lista_de_caminhos(args);
                let t = diff_do_repo(
                    &self.bwrap,
                    &ctx.workdir,
                    repo,
                    texto(args, "rev"),
                    booleano(args, "cached"),
                    &caminhos,
                    self.timeout.min(ctx.timeout),
                )
                .await?;
                let (t, cortou) = limitar_diff(&t, DIFF_MAX_BYTES);
                Ok(json!({"arquivos": analisar_diff(&t), "truncado": cortou}))
            }
            "log" => {
                let mut a = s(&["log", "--no-color"]);
                a.push(format!("--max-count={}", inteiro(args, "limit", 20, 200)));
                a.push(format!("--format={FORMATO_COMMIT}"));
                if let Some(r) = texto(args, "rev") {
                    a.push(referencia(r)?);
                }
                a.push("--".into());
                a.extend(lista_de_caminhos(args));
                let o = self.git(ctx, repo, a).await?;
                Ok(json!({"commits": analisar_commits(&o)}))
            }
            "show" => {
                let rev = referencia(texto(args, "rev").unwrap_or("HEAD"))?;
                let mut a = s(&["show", "--no-color", "--no-ext-diff", "--no-textconv", "-M"]);
                a.push(format!("--format={FORMATO_COMMIT}"));
                a.push(rev);
                let o = self.git(ctx, repo, a).await?;
                let (cab, diff) = o.split_once(RS).unwrap_or((&o, ""));
                let (diff, cortou) = limitar_diff(diff, DIFF_MAX_BYTES);
                let commit = analisar_commits(&format!("{cab}{RS}")).into_iter().next();
                Ok(json!({"commit": commit, "arquivos": analisar_diff(&diff), "truncado": cortou}))
            }
            "blame" => {
                let arq = exigir(args, "file")?;
                let mut a = s(&["blame", "--porcelain", "--no-textconv"]);
                if let (Some(i), f) = (
                    args.get("start_line").and_then(Value::as_u64),
                    args.get("end_line").and_then(Value::as_u64),
                ) {
                    a.push(format!("-L{i},{}", f.unwrap_or(i + 49)));
                }
                if let Some(r) = texto(args, "rev") {
                    a.push(referencia(r)?);
                }
                a.push("--".into());
                a.push(arq.to_string());
                let o = self.git(ctx, repo, a).await?;
                let mut l = analisar_blame(&o);
                let total = l.len();
                l.truncate(500);
                Ok(json!({"arquivo": arq, "linhas": l, "total": total}))
            }
            "branches" => {
                let o = self
                    .git(
                        ctx,
                        repo,
                        s(&["branch", "--list", "--no-color", "--format=%(HEAD)%1f%(refname:short)%1f%(objectname:short)%1f%(upstream:short)"]),
                    )
                    .await?;
                let ramos: Vec<Value> = o
                    .lines()
                    .filter_map(|l| {
                        let c: Vec<&str> = l.split(US).collect();
                        (c.len() >= 3).then(|| json!({"ramo": c[1], "atual": c[0] == "*", "commit": c[2], "upstream": c.get(3).filter(|u| !u.is_empty())}))
                    })
                    .collect();
                Ok(json!({"ramos": ramos}))
            }
            outra => Err(ToolError::InvalidArguments(format!(
                "action desconhecida em git: {outra} (status, diff, log, show, blame, branches; \
                 mudancas ficam no git_write)"
            ))),
        }
    }

    async fn escrever(
        &self,
        ctx: &ToolContext,
        repo: &str,
        acao: &str,
        args: &Value,
    ) -> Result<Value, ToolError> {
        let s = |v: &[&str]| v.iter().map(|x| x.to_string()).collect::<Vec<_>>();
        match acao {
            "init" => {
                let mut a = s(&["init", "-q"]);
                if let Some(b) = texto(args, "branch") {
                    a.push(format!("--initial-branch={}", nome_de_ramo(b)?));
                }
                // A pasta pode nao existir ainda: vai como argumento do init, nao no `-C`.
                let alvo = if repo.is_empty() { "." } else { repo };
                a.push("--".into());
                a.push(alvo.into());
                self.git(ctx, "", a).await?;
                Ok(json!({"iniciado": alvo}))
            }
            "add" => {
                let caminhos = lista_de_caminhos(args);
                if caminhos.is_empty() {
                    return Err(ToolError::InvalidArguments(
                        "falta 'paths' (use [\".\"] para tudo)".into(),
                    ));
                }
                let varredura = self
                    .varrer(ctx, repo, crate::segredos::Gravacao::Add(caminhos.clone()))
                    .await?;
                let mut a = s(&["add", "--"]);
                a.extend(caminhos);
                self.git(ctx, repo, a).await?;
                let mut st = self.ler(ctx, repo, "status", args).await?;
                st["varredura_de_segredos"] = varredura;
                Ok(st)
            }
            "commit" => {
                let msg = exigir(args, "message")?;
                let todos = booleano(args, "all");
                let varredura = self
                    .varrer(
                        ctx,
                        repo,
                        crate::segredos::Gravacao::Commit {
                            todos,
                            mensagem: msg.to_string(),
                        },
                    )
                    .await?;
                let mut a = s(&["commit", "-q", "--no-verify"]);
                if todos {
                    a.push("-a".into());
                }
                a.push("-m".into());
                a.push(msg.to_string());
                self.git(ctx, repo, a).await?;
                let o = self
                    .git(
                        ctx,
                        repo,
                        vec![
                            "log".into(),
                            "--max-count=1".into(),
                            format!("--format={FORMATO_COMMIT}"),
                        ],
                    )
                    .await?;
                Ok(json!({"commit": analisar_commits(&o).into_iter().next(),
                          "varredura_de_segredos": varredura}))
            }
            "checkout" => {
                let b = nome_de_ramo(exigir(args, "branch")?)?;
                let mut a = s(&["switch", "-q"]);
                if booleano(args, "create") {
                    a.push("-c".into());
                }
                a.push(b.clone());
                if let Some(base) = texto(args, "base") {
                    a.push(nome_de_ramo(base)?);
                }
                self.git(ctx, repo, a).await?;
                Ok(json!({"ramo_atual": b}))
            }
            "branch_create" => {
                let b = nome_de_ramo(exigir(args, "branch")?)?;
                let mut a = vec!["branch".to_string(), b.clone()];
                if let Some(base) = texto(args, "base") {
                    a.push(nome_de_ramo(base)?);
                }
                self.git(ctx, repo, a).await?;
                Ok(json!({"criado": b}))
            }
            "branch_delete" => {
                let b = nome_de_ramo(exigir(args, "branch")?)?;
                // `-d` recusa ramo nao integrado; apagar trabalho nao integrado e pedido
                // explicito (`force`).
                let flag = if booleano(args, "force") { "-D" } else { "-d" };
                self.git(ctx, repo, vec!["branch".into(), flag.into(), b.clone()])
                    .await?;
                Ok(json!({"apagado": b}))
            }
            "stash" => {
                let op = texto(args, "op").unwrap_or("push");
                let idx = args
                    .get("index")
                    .and_then(Value::as_u64)
                    .map(|i| format!("stash@{{{i}}}"));
                let mut varredura = Value::Null;
                let a: Vec<String> = match op {
                    "push" => {
                        // O stash grava commit: passa pela mesma varredura do `commit`.
                        varredura = self
                            .varrer(
                                ctx,
                                repo,
                                crate::segredos::Gravacao::Stash {
                                    nao_rastreados: booleano(args, "include_untracked"),
                                    mensagem: texto(args, "message").map(str::to_string),
                                },
                            )
                            .await?;
                        let mut a = s(&["stash", "push", "-q"]);
                        if booleano(args, "include_untracked") {
                            a.push("-u".into());
                        }
                        if let Some(m) = texto(args, "message") {
                            a.push("-m".into());
                            a.push(m.to_string());
                        }
                        a
                    }
                    "pop" | "apply" | "drop" => {
                        let mut a = vec!["stash".to_string(), op.to_string(), "-q".into()];
                        a.extend(idx);
                        a
                    }
                    "list" => s(&["stash", "list", "--format=%gd%x1f%s"]),
                    outra => {
                        return Err(ToolError::InvalidArguments(format!(
                            "op desconhecida em stash: {outra} (push, pop, apply, drop, list)"
                        )));
                    }
                };
                let o = self.git(ctx, repo, a).await?;
                let lista: Vec<Value> = o
                    .lines()
                    .filter_map(|l| l.split_once(US))
                    .map(|(r, m)| json!({"ref": r, "mensagem": m}))
                    .collect();
                let status = self.ler(ctx, repo, "status", args).await?;
                let mut r = json!({"op": op, "pilha": lista, "status": status});
                if !varredura.is_null() {
                    r["varredura_de_segredos"] = varredura;
                }
                Ok(r)
            }
            "add_trecho" => {
                let arquivo = exigir(args, "file")?.to_string();
                let trecho = match (
                    args.get("hunk"),
                    args.get("start_line").and_then(Value::as_u64),
                    args.get("end_line").and_then(Value::as_u64),
                ) {
                    (Some(h), _, _) => {
                        let idx: Vec<usize> = match h {
                            Value::Array(a) => a
                                .iter()
                                .filter_map(|v| v.as_u64())
                                .map(|v| v as usize)
                                .collect(),
                            v => v.as_u64().map(|v| v as usize).into_iter().collect(),
                        };
                        if idx.is_empty() {
                            return Err(ToolError::InvalidArguments(
                                "'hunk' e um indice (ou lista) do diff do arquivo".into(),
                            ));
                        }
                        Trecho::Hunks(idx)
                    }
                    (None, Some(i), f) => Trecho::Linhas(i, f.unwrap_or(i)),
                    _ => {
                        return Err(ToolError::InvalidArguments(
                            "falta 'hunk' (indice do trecho no git diff) ou 'start_line'/'end_line'"
                                .into(),
                        ));
                    }
                };
                let t = diff_do_repo(
                    &self.bwrap,
                    &ctx.workdir,
                    repo,
                    None,
                    false,
                    std::slice::from_ref(&arquivo),
                    self.timeout.min(ctx.timeout),
                )
                .await?;
                let arqs = analisar_diff(&t);
                let Some(arq) = arqs.into_iter().find(|a| a.caminho == arquivo) else {
                    return Err(ToolError::InvalidArguments(format!(
                        "{arquivo}: sem diff contra o indice (arquivo novo ou sem mudanca: use add)"
                    )));
                };
                let patch = patch_do_trecho(&arq, &trecho)?;
                // O trecho e o que sera gravado: a varredura le o proprio patch.
                let varredura = self
                    .varrer(
                        ctx,
                        repo,
                        crate::segredos::Gravacao::Trecho {
                            patch: patch.clone(),
                        },
                    )
                    .await?;
                // O patch entra por heredoc com marca unica: quoted, nada se expande.
                let marca = format!("FIM_DO_TRECHO_{}", phxclaw_types::new_uuid_v7());
                let s = rodar_roteiro_git(
                    &self.bwrap,
                    &ctx.workdir,
                    repo,
                    |git| {
                        format!(
                            "{git} apply --cached --whitespace=nowarn <<'{marca}'\n{patch}{marca}\n"
                        )
                    },
                    self.timeout.min(ctx.timeout),
                    DIFF_MAX_BYTES,
                )
                .await?;
                exigir_sucesso("apply --cached", s)?;
                let no_indice = diff_do_repo(
                    &self.bwrap,
                    &ctx.workdir,
                    repo,
                    None,
                    true,
                    std::slice::from_ref(&arquivo),
                    self.timeout.min(ctx.timeout),
                )
                .await?;
                Ok(json!({
                    "arquivo": arquivo,
                    "no_indice": analisar_diff(&no_indice).into_iter().next(),
                    "varredura_de_segredos": varredura,
                }))
            }
            "merge" => {
                // So merge, nunca rebase nem `--force`: o ramo alheio nao se reescreve.
                let b = nome_de_ramo(exigir(args, "branch")?)?;
                let r = rodar_git(
                    &self.bwrap,
                    &ctx.workdir,
                    repo,
                    vec!["merge".into(), "--no-edit".into(), b.clone()],
                    self.timeout.min(ctx.timeout),
                )
                .await?;
                if r.exit_code == Some(0) {
                    let o = self
                        .git(
                            ctx,
                            repo,
                            vec![
                                "log".into(),
                                "--max-count=1".into(),
                                format!("--format={FORMATO_COMMIT}"),
                            ],
                        )
                        .await?;
                    return Ok(json!({"ramo": b, "concluido": true,
                                     "commit": analisar_commits(&o).into_iter().next()}));
                }
                let em_conflito = self.arquivos_em_conflito(ctx, repo).await?;
                if em_conflito.is_empty() {
                    return exigir_sucesso("merge", r).map(|_| Value::Null);
                }
                Ok(
                    json!({"ramo": b, "concluido": false, "conflitos": em_conflito,
                          "proximo": "git_write conflitos / resolver, depois commit; ou abort"}),
                )
            }
            "conflitos" => {
                let mut lista = Vec::new();
                for arquivo in self.arquivos_em_conflito(ctx, repo).await? {
                    let texto = self.ler_da_arvore(ctx, repo, &arquivo)?;
                    lista.push(json!({"arquivo": arquivo, "blocos": blocos_de_conflito(&texto)}));
                }
                Ok(json!({"arquivos": lista}))
            }
            "resolver" => {
                let arquivo = exigir(args, "file")?.to_string();
                let bloco = args.get("block").and_then(Value::as_u64).ok_or_else(|| {
                    ToolError::InvalidArguments("falta 'block' (indice do bloco)".into())
                })? as usize;
                let escolha = exigir(args, "choice")?;
                let texto = self.ler_da_arvore(ctx, repo, &arquivo)?;
                let linhas: Vec<String> = match escolha {
                    "nosso" | "deles" => {
                        let b = blocos_de_conflito(&texto)
                            .into_iter()
                            .find(|b| b.indice == bloco)
                            .ok_or_else(|| {
                                ToolError::InvalidArguments(format!(
                                    "bloco {bloco} nao existe em {arquivo}"
                                ))
                            })?;
                        if escolha == "nosso" { b.nosso } else { b.deles }
                    }
                    "texto" => exigir(args, "text")?.lines().map(str::to_string).collect(),
                    outra => {
                        return Err(ToolError::InvalidArguments(format!(
                            "choice desconhecida: {outra} (nosso, deles, texto)"
                        )));
                    }
                };
                let (novo, restantes) = resolver_bloco(&texto, bloco, &linhas)?;
                let rel = if repo.is_empty() {
                    arquivo.clone()
                } else {
                    format!("{repo}/{arquivo}")
                };
                let alvo = crate::tarefa::confine(&ctx.workdir, &rel).map_err(ToolError::Denied)?;
                std::fs::write(&alvo, novo).map_err(|e| ToolError::Failed(e.to_string()))?;
                let mut r =
                    json!({"arquivo": arquivo, "blocos_restantes": restantes, "no_indice": false});
                if restantes == 0 {
                    // Resolvido inteiro: vai ao indice pela mesma varredura do `add`.
                    let varredura = self
                        .varrer(
                            ctx,
                            repo,
                            crate::segredos::Gravacao::Add(vec![arquivo.clone()]),
                        )
                        .await?;
                    self.git(ctx, repo, vec!["add".into(), "--".into(), arquivo])
                        .await?;
                    r["no_indice"] = json!(true);
                    r["varredura_de_segredos"] = varredura;
                }
                Ok(r)
            }
            "abort" => {
                self.git(ctx, repo, s(&["merge", "--abort"])).await?;
                Ok(self.ler(ctx, repo, "status", args).await?)
            }
            outra => Err(ToolError::InvalidArguments(format!(
                "action desconhecida em git_write: {outra} (init, add, add_trecho, commit, \
                 checkout, branch_create, branch_delete, stash, merge, conflitos, resolver, abort)"
            ))),
        }
    }

    /// Os caminhos sem resolver (`--diff-filter=U`), na ordem do git.
    async fn arquivos_em_conflito(
        &self,
        ctx: &ToolContext,
        repo: &str,
    ) -> Result<Vec<String>, ToolError> {
        let o = self
            .git(
                ctx,
                repo,
                vec![
                    "diff".into(),
                    "--name-only".into(),
                    "--diff-filter=U".into(),
                    "-z".into(),
                ],
            )
            .await?;
        Ok(o.split('\0')
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .collect())
    }

    /// O arquivo da arvore de trabalho, lido do hospedeiro (o mesmo lugar que o sandbox ve
    /// como /work), confinado a pasta da tarefa.
    fn ler_da_arvore(
        &self,
        ctx: &ToolContext,
        repo: &str,
        arquivo: &str,
    ) -> Result<String, ToolError> {
        let rel = if repo.is_empty() {
            arquivo.to_string()
        } else {
            format!("{repo}/{arquivo}")
        };
        let p = crate::tarefa::confine(&ctx.workdir, &rel).map_err(ToolError::Denied)?;
        std::fs::read_to_string(&p).map_err(|e| ToolError::Failed(format!("{rel}: {e}")))
    }
}

impl Tool for GitTool {
    fn spec(&self) -> ToolSpec {
        if self.escrita {
            ToolSpec {
                name: "git_write".into(),
                description: "Change a git repository in the task directory (sandboxed, no \
network). action: init {branch?}; add {paths}; add_trecho {file, hunk (index or list from \
git diff) | start_line, end_line} stages only that part of the file; commit {message, \
all?}; checkout {branch, create?, base?}; branch_create {branch, base?}; branch_delete \
{branch, force?}; stash {op: push|pop|apply|drop|list, message?, include_untracked?, \
index?}; merge {branch} (never rebase/force); conflitos (lists <<<<<<< blocks with both \
sides); resolver {file, block, choice: nosso|deles|texto, text?}; abort. 'path' selects \
the repo folder (default: task root)."
                    .into(),
                parameters: json!({"type":"object","properties":{
                    "action":{"type":"string","enum":["init","add","add_trecho","commit","checkout","branch_create","branch_delete","stash","merge","conflitos","resolver","abort"]},
                    "file":{"type":"string"},
                    "hunk":{"type":["integer","array"],"items":{"type":"integer"}},
                    "start_line":{"type":"integer"},
                    "end_line":{"type":"integer"},
                    "block":{"type":"integer"},
                    "choice":{"type":"string","enum":["nosso","deles","texto"]},
                    "text":{"type":"string"},
                    "path":{"type":"string"},
                    "paths":{"type":"array","items":{"type":"string"}},
                    "message":{"type":"string"},
                    "all":{"type":"boolean"},
                    "branch":{"type":"string"},
                    "base":{"type":"string"},
                    "create":{"type":"boolean"},
                    "force":{"type":"boolean"},
                    "op":{"type":"string"},
                    "include_untracked":{"type":"boolean"},
                    "index":{"type":"integer"}
                },"required":["action"]}),
            }
        } else {
            ToolSpec {
                name: "git".into(),
                description: "Read a git repository in the task directory, structured JSON. \
action: status; diff {rev?, cached?, paths?} (files and hunks); log {rev?, limit?, paths?}; \
show {rev}; blame {file, start_line?, end_line?, rev?}; branches. 'path' selects the repo \
folder (default: task root)."
                    .into(),
                parameters: json!({"type":"object","properties":{
                    "action":{"type":"string","enum":["status","diff","log","show","blame","branches"]},
                    "path":{"type":"string"},
                    "rev":{"type":"string"},
                    "cached":{"type":"boolean"},
                    "paths":{"type":"array","items":{"type":"string"}},
                    "limit":{"type":"integer"},
                    "file":{"type":"string"},
                    "start_line":{"type":"integer"},
                    "end_line":{"type":"integer"}
                },"required":["action"]}),
            }
        }
    }
    fn capability(&self) -> &'static str {
        if self.escrita {
            "git.write"
        } else {
            "git.read"
        }
    }
    fn run<'a>(
        &'a self,
        args: Value,
        ctx: &'a ToolContext,
    ) -> BoxFut<'a, Result<ToolOutput, ToolError>> {
        Box::pin(async move {
            let acao = exigir(&args, "action")?.to_string();
            let repo = caminho_do_projeto(&ctx.workdir, texto(&args, "path").unwrap_or(""))?;
            let v = if self.escrita {
                self.escrever(ctx, &repo, &acao, &args).await?
            } else {
                self.ler(ctx, &repo, &acao, &args).await?
            };
            Ok(ToolOutput::text(v.to_string()))
        })
    }
}

/// `git_worktree` (git.write): uma pasta de trabalho por tarefa paralela, em
/// `<repo>/.worktrees/<nome>`, no ramo `phxclaw/<nome>`.
///
/// Dentro da pasta da tarefa porque o sandbox so ve /work; e o `.git/info/exclude` ganha
/// `/.worktrees/` para o `status` do repositorio principal nao listar as outras arvores
/// como arquivo novo. O git 2.43 grava o caminho ABSOLUTO (/work/...) no `.git` da arvore,
/// entao ela so vale la dentro -- que e onde todo git do agente roda.
pub struct WorktreeTool {
    pub bwrap: PathBuf,
    pub timeout: Duration,
}

fn nome_de_arvore(s: &str) -> Result<String, ToolError> {
    let s = s.trim();
    if s.is_empty()
        || s.starts_with(['-', '.'])
        || !s
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "._-".contains(c))
    {
        return Err(ToolError::InvalidArguments(format!(
            "nome de worktree invalido: {s:?} (letras, digitos, . _ -; sem '-' ou '.' no inicio)"
        )));
    }
    Ok(s.to_string())
}

impl Tool for WorktreeTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "git_worktree".into(),
            description: "Isolated git worktrees for parallel tasks. action=add {name, branch?, \
base?} creates <repo>/.worktrees/<name> on branch phxclaw/<name> (returns its path, usable as \
'path' in git/git_write and file tools); action=list; action=remove {name, force?}. 'path' \
selects the main repo."
                .into(),
            parameters: json!({"type":"object","properties":{
                "action":{"type":"string","enum":["add","list","remove"]},
                "path":{"type":"string"},
                "name":{"type":"string"},
                "branch":{"type":"string"},
                "base":{"type":"string"},
                "force":{"type":"boolean"}
            },"required":["action"]}),
        }
    }
    fn capability(&self) -> &'static str {
        "git.write"
    }
    fn run<'a>(
        &'a self,
        args: Value,
        ctx: &'a ToolContext,
    ) -> BoxFut<'a, Result<ToolOutput, ToolError>> {
        Box::pin(async move {
            let repo = caminho_do_projeto(&ctx.workdir, texto(&args, "path").unwrap_or(""))?;
            let prazo = self.timeout.min(ctx.timeout);
            let git = |a: Vec<String>| {
                let repo = repo.clone();
                async move {
                    let acao = a.get(1).cloned().unwrap_or_default();
                    exigir_sucesso(
                        &format!("worktree {acao}"),
                        rodar_git(&self.bwrap, &ctx.workdir, &repo, a, prazo).await?,
                    )
                }
            };
            let v = match exigir(&args, "action")? {
                "list" => {
                    let o =
                        git(vec!["worktree".into(), "list".into(), "--porcelain".into()]).await?;
                    json!({"worktrees": analisar_worktrees(&o)})
                }
                "add" => {
                    let nome = nome_de_arvore(exigir(&args, "name")?)?;
                    let ramo =
                        nome_de_ramo(texto(&args, "branch").unwrap_or(&format!("phxclaw/{nome}")))?;
                    let dir = format!(".worktrees/{nome}");
                    let mut a: Vec<String> = vec![
                        "worktree".into(),
                        "add".into(),
                        "-q".into(),
                        "-b".into(),
                        ramo.clone(),
                        dir.clone(),
                    ];
                    if let Some(b) = texto(&args, "base") {
                        a.push(nome_de_ramo(b)?);
                    }
                    git(a).await?;
                    // O exclude mora no diretorio COMUM do git (vale para todas as arvores).
                    let ex = git(vec![
                        "rev-parse".into(),
                        "--git-path".into(),
                        "info/exclude".into(),
                    ])
                    .await?;
                    excluir_worktrees(&ctx.workdir, &repo, ex.trim())?;
                    let caminho = if repo.is_empty() {
                        dir
                    } else {
                        format!("{repo}/{dir}")
                    };
                    json!({"path": caminho, "ramo": ramo})
                }
                "remove" => {
                    let nome = nome_de_arvore(exigir(&args, "name")?)?;
                    let mut a: Vec<String> = vec!["worktree".into(), "remove".into()];
                    // Sem `force` o git recusa arvore com mudanca nao gravada: e o que impede
                    // a tarefa paralela de perder o trabalho dela ao ser recolhida.
                    if booleano(&args, "force") {
                        a.push("--force".into());
                    }
                    a.push(format!(".worktrees/{nome}"));
                    git(a).await?;
                    git(vec!["worktree".into(), "prune".into()]).await?;
                    json!({"removido": nome, "ramo_mantido": true})
                }
                outra => {
                    return Err(ToolError::InvalidArguments(format!(
                        "action desconhecida em git_worktree: {outra} (add, list, remove)"
                    )));
                }
            };
            Ok(ToolOutput::text(v.to_string()))
        })
    }
}

/// Acrescenta `/.worktrees/` ao exclude, se faltar. O caminho vem do git (relativo ao repo,
/// ou absoluto em /work) e volta ao hospedeiro pelo `confine` da pasta da tarefa.
fn excluir_worktrees(workdir: &Path, repo: &str, caminho: &str) -> Result<(), ToolError> {
    let rel = match caminho.strip_prefix("/work/") {
        Some(r) => r.to_string(),
        None if repo.is_empty() => caminho.to_string(),
        None => format!("{repo}/{caminho}"),
    };
    let alvo = crate::tarefa::confine(workdir, &rel).map_err(ToolError::Denied)?;
    let atual = std::fs::read_to_string(&alvo).unwrap_or_default();
    if atual.lines().any(|l| l.trim() == "/.worktrees/") {
        return Ok(());
    }
    if let Some(p) = alvo.parent() {
        std::fs::create_dir_all(p).map_err(|e| ToolError::Failed(e.to_string()))?;
    }
    let sep = if atual.is_empty() || atual.ends_with('\n') {
        ""
    } else {
        "\n"
    };
    std::fs::write(&alvo, format!("{atual}{sep}/.worktrees/\n"))
        .map_err(|e| ToolError::Failed(e.to_string()))
}

/// As ferramentas da frente de codigo, num lugar so para a `Montagem`: git (sem bwrap nao
/// existe, como o `shell`), busca, caderno, pontos de restauracao e revisao.
pub fn ferramentas_de_codigo(
    store: &crate::tarefa::TaskStore,
    pasta_do_agente: &Path,
    llm: std::sync::Arc<dyn phxclaw_agent_core::Llm>,
) -> Vec<std::sync::Arc<dyn Tool>> {
    use std::sync::Arc;
    let bwrap = crate::arquivos::achar_bwrap();
    let mut v: Vec<Arc<dyn Tool>> = vec![
        Arc::new(crate::busca::GlobTool),
        Arc::new(crate::busca::GrepTool),
        Arc::new(crate::busca::ReplaceInProjectTool),
        Arc::new(crate::historico::FileHistoryTool { escrita: false }),
        Arc::new(crate::historico::FileHistoryTool { escrita: true }),
        Arc::new(crate::notebook::NotebookReadTool),
        Arc::new(crate::notebook::NotebookEditTool),
        Arc::new(crate::checkpoint::CheckpointTool {
            store: store.clone(),
            restaurar: false,
        }),
        Arc::new(crate::checkpoint::CheckpointTool {
            store: store.clone(),
            restaurar: true,
        }),
        Arc::new(crate::revisao::CodeReviewTool {
            llm,
            bwrap: bwrap.clone(),
        }),
    ];
    if let Some(b) = bwrap {
        v.push(Arc::new(GitTool::leitura(b.clone())));
        v.push(Arc::new(GitTool::escrita(b.clone())));
        // Tarefas do projeto CONFIADO no mesmo bwrap do shell, resolvidas aqui uma vez: as
        // regras de comando e a corrida leem a mesma lista.
        v.push(Arc::new(crate::projeto_tarefas::ProjectTaskTool {
            bwrap: b.clone(),
            timeout: Duration::from_secs(600),
            tarefas: crate::projeto_tarefas::do_projeto_confiado(pasta_do_agente),
        }));
        v.push(Arc::new(WorktreeTool {
            bwrap: b,
            timeout: Duration::from_secs(60),
        }));
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    const DIFF: &str = "diff --git a/src/a.rs b/src/a.rs
index 1..2 100644
--- a/src/a.rs
+++ b/src/a.rs
@@ -1,2 +1,3 @@ fn main()
 linha1
--- conteudo que parece cabecalho
+novo
+++ tambem conteudo
@@ -10 +11,2 @@
-velha
+nova1
+nova2
diff --git a/b.txt b/b.txt
new file mode 100644
--- /dev/null
+++ b/b.txt
@@ -0,0 +1 @@
+oi
diff --git a/c.bin b/c.bin
Binary files a/c.bin and b/c.bin differ
diff --git a/x.txt b/y.txt
similarity index 90%
rename from x.txt
rename to y.txt
";

    #[test]
    fn diff_vira_arquivos_e_hunks_sem_confundir_conteudo_com_cabecalho() {
        let v = analisar_diff(DIFF);
        assert_eq!(v.len(), 4, "{v:#?}");
        assert_eq!(v[0].caminho, "src/a.rs");
        assert_eq!(v[0].hunks.len(), 2);
        assert_eq!(v[0].hunks[0].contexto, "fn main()");
        // `--- conteudo` e `+++ tambem` estao DENTRO do hunk: sao remocao e adicao
        assert_eq!((v[0].adicoes, v[0].remocoes), (4, 2), "{:#?}", v[0]);
        assert_eq!(v[0].hunks[1].novo_inicio, 11);
        assert!(v[0].linha_no_diff(12) && !v[0].linha_no_diff(13) && !v[0].linha_no_diff(9));
        assert_eq!(
            (v[1].caminho.as_str(), v[1].estado.as_str()),
            ("b.txt", "adicionado")
        );
        assert!(v[2].binario);
        assert_eq!(
            (
                v[3].estado.as_str(),
                v[3].caminho.as_str(),
                v[3].antigo.as_deref()
            ),
            ("renomeado", "y.txt", Some("x.txt"))
        );
    }

    #[test]
    fn referencia_que_seria_opcao_e_recusada() {
        assert!(referencia("--output=/work/x").is_err());
        assert!(referencia("-p").is_err());
        assert!(referencia("main; rm -rf /").is_err());
        assert_eq!(referencia(" HEAD~2 ").unwrap(), "HEAD~2");
        assert!(referencia("origin/main..topico").is_ok());
    }

    #[test]
    fn ramo_so_de_nome_ou_sha() {
        for ok in ["main", "phxclaw/tarefa-1", "v1.2", "0a1b2c3d"] {
            assert!(nome_de_ramo(ok).is_ok(), "{ok}");
        }
        for nao in [
            "stash@{0}^3",
            "HEAD~1",
            "main^",
            "main:a",
            "a..b",
            "@",
            "-x",
            ".x",
        ] {
            assert!(nome_de_ramo(nao).is_err(), "{nao}");
        }
    }

    #[test]
    fn status_porcelana_z_com_rename_e_contagem() {
        let s =
            "## main...origin/main [ahead 2, behind 1]\0M  a.rs\0R  novo.rs\0velho.rs\0?? n.txt\0";
        let v = analisar_status(s);
        assert_eq!(v["ramo"], "main");
        assert_eq!(
            (v["a_frente"].as_u64(), v["atras"].as_u64()),
            (Some(2), Some(1))
        );
        let a = v["arquivos"].as_array().unwrap();
        assert_eq!(a.len(), 3, "{v}");
        assert_eq!(a[1]["de"], "velho.rs");
        assert_eq!(a[2]["estado"], "nao_rastreado");
    }

    #[test]
    fn diff_grande_corta_em_fronteira_de_arquivo() {
        let um = "diff --git a/a b/a\n--- a/a\n+++ b/a\n@@ -1 +1 @@\n-x\n+y\n";
        let t = um.repeat(10);
        let (c, cortou) = limitar_diff(&t, um.len() * 3 + 5);
        assert!(cortou);
        assert_eq!(c, um.repeat(3));
    }
}
