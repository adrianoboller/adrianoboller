//! Hooks: comandos do operador disparados pelos eventos do laco do agente, no formato do
//! Claude Code (`.phxclaw/hooks.json`):
//!
//! ```json
//! {"hooks": {"PreToolUse": [{"matcher": "shell|write_file",
//!                            "hooks": [{"type": "command", "command": "/hooks/guarda.sh"}]}]}}
//! ```
//!
//! Eventos: `PreToolUse`, `PostToolUse`, `TaskStart`, `TaskEnd` e `Stop` (antes de a
//! resposta final ser aceita). O comando recebe o evento em JSON no stdin e, como no
//! Claude Code, **sai com 2 para bloquear**: o stderr vira o motivo que o modelo le. Outro
//! codigo diferente de zero e aviso, nao bloqueio.
//!
//! Decisoes que valem saber:
//!
//! - **Roda no MESMO bwrap do `shell`**, montado pela mesma funcao, com a pasta da tarefa
//!   em `/work` e a pasta do `hooks.json` em `/hooks`, so leitura. Hook fora do sandbox
//!   seria o unico codigo do agente com o disco inteiro do hospedeiro.
//! - **O arquivo e do operador, nunca da tarefa.** Ele mora na pasta do projeto, fora do
//!   `work/` que o shell do modelo enxerga: hook que o modelo pudesse reescrever deixaria
//!   de guardar qualquer coisa.
//! - **Guarda configurada que nao roda, fecha.** `PreToolUse` sem bwrap ou com arquivo
//!   ilegivel bloqueia a ferramenta dizendo por que; os outros eventos so avisam.

use phxclaw_sandbox::{SandboxExtras, WorkdirCommand, run_in_workdir_com};
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Evento {
    AntesDaFerramenta,
    DepoisDaFerramenta,
    InicioDaTarefa,
    FimDaTarefa,
    AntesDeResponder,
}

impl Evento {
    pub fn nome(self) -> &'static str {
        match self {
            Self::AntesDaFerramenta => "PreToolUse",
            Self::DepoisDaFerramenta => "PostToolUse",
            Self::InicioDaTarefa => "TaskStart",
            Self::FimDaTarefa => "TaskEnd",
            Self::AntesDeResponder => "Stop",
        }
    }

    fn do_nome(n: &str) -> Option<Self> {
        [
            Self::AntesDaFerramenta,
            Self::DepoisDaFerramenta,
            Self::InicioDaTarefa,
            Self::FimDaTarefa,
            Self::AntesDeResponder,
        ]
        .into_iter()
        .find(|e| e.nome() == n)
    }
}

#[derive(Debug, Clone, Deserialize)]
struct ComandoBruto {
    #[serde(default, rename = "type")]
    tipo: Option<String>,
    command: String,
    #[serde(default)]
    timeout: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
struct GrupoBruto {
    #[serde(default)]
    matcher: Option<String>,
    hooks: Vec<ComandoBruto>,
}

#[derive(Debug, Clone, Deserialize)]
struct ArquivoBruto {
    #[serde(default)]
    hooks: BTreeMap<String, Vec<GrupoBruto>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Comando {
    /// Nomes de ferramenta separados por `|`; vazio ou `*` casa todas.
    pub matcher: Option<String>,
    pub linha: String,
    pub prazo: Duration,
}

#[derive(Debug, Clone)]
pub struct Hooks {
    /// Pasta do `hooks.json`, montada em `/hooks` so leitura.
    pub pasta: PathBuf,
    pub bwrap: Option<PathBuf>,
    pub comandos: BTreeMap<Evento, Vec<Comando>>,
    /// Arquivo que existe e nao se leu: a guarda fecha em vez de sumir.
    pub erro: Option<String>,
}

/// O que os hooks de um evento disseram.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Saida {
    /// Algum saiu com 2 (ou a guarda nao pode rodar): o motivo.
    pub bloqueio: Option<String>,
    /// stdout dos que sairam com 0, na ordem: contexto que o `TaskStart` acrescenta.
    pub contexto: Vec<String>,
    /// Falhas que nao bloqueiam (codigo diferente de 0 e 2, prazo estourado).
    pub avisos: Vec<String>,
}

/// Teto do JSON de entrada: vai por variavel de ambiente, e o Linux recusa variavel maior
/// que 128 KiB (`MAX_ARG_STRLEN`) com um `E2BIG` que mataria o hook antes de rodar.
const ENTRADA_MAX: usize = 96 * 1024;

impl Hooks {
    pub fn de_json(texto: &str, pasta: PathBuf, bwrap: Option<PathBuf>) -> Result<Self, String> {
        let b: ArquivoBruto = serde_json::from_str(texto).map_err(|e| e.to_string())?;
        let mut comandos: BTreeMap<Evento, Vec<Comando>> = BTreeMap::new();
        for (nome, grupos) in b.hooks {
            let ev = Evento::do_nome(&nome).ok_or_else(|| {
                format!("evento desconhecido: {nome} (PreToolUse, PostToolUse, TaskStart, TaskEnd, Stop)")
            })?;
            for g in grupos {
                for c in g.hooks {
                    if c.tipo.as_deref().is_some_and(|t| t != "command") {
                        return Err(format!("tipo de hook nao suportado: {:?}", c.tipo));
                    }
                    if c.command.trim().is_empty() {
                        return Err(format!("{nome}: comando vazio"));
                    }
                    comandos.entry(ev).or_default().push(Comando {
                        matcher: g.matcher.clone().filter(|m| !m.trim().is_empty()),
                        linha: c.command,
                        prazo: Duration::from_secs(c.timeout.unwrap_or(60).clamp(1, 600)),
                    });
                }
            }
        }
        Ok(Self {
            pasta,
            bwrap,
            comandos,
            erro: None,
        })
    }

    /// `<pasta>/hooks.json`; ausente e `None`. Ilegivel vira hooks que fecham.
    pub fn carregar(pasta: &Path, bwrap: Option<PathBuf>) -> Option<Self> {
        let arquivo = pasta.join("hooks.json");
        match std::fs::read_to_string(&arquivo) {
            Ok(t) => Some(
                Self::de_json(&t, pasta.to_path_buf(), bwrap.clone()).unwrap_or_else(|e| Self {
                    pasta: pasta.to_path_buf(),
                    bwrap,
                    comandos: BTreeMap::new(),
                    erro: Some(format!("{}: {e}", arquivo.display())),
                }),
            ),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => Some(Self {
                pasta: pasta.to_path_buf(),
                bwrap,
                comandos: BTreeMap::new(),
                erro: Some(format!("{}: {e}", arquivo.display())),
            }),
        }
    }

    pub fn tem(&self, ev: Evento) -> bool {
        self.comandos.get(&ev).is_some_and(|v| !v.is_empty())
            || (self.erro.is_some() && ev == Evento::AntesDaFerramenta)
    }

    /// Dispara os comandos do evento que casam com `ferramenta`, em ordem; o primeiro
    /// bloqueio para os seguintes. Bloqueante: o motor chama por `spawn_blocking`.
    pub fn disparar(
        &self,
        ev: Evento,
        ferramenta: Option<&str>,
        workdir: &Path,
        entrada: &Value,
    ) -> Saida {
        let mut s = Saida::default();
        if let Some(e) = &self.erro {
            if ev == Evento::AntesDaFerramenta {
                s.bloqueio = Some(format!("hooks ilegiveis, guarda fechada: {e}"));
            } else {
                s.avisos.push(e.clone());
            }
            return s;
        }
        let Some(lista) = self.comandos.get(&ev) else {
            return s;
        };
        let lista: Vec<&Comando> = lista
            .iter()
            .filter(|c| casa(c.matcher.as_deref(), ferramenta))
            .collect();
        if lista.is_empty() {
            return s;
        }
        let Some(bwrap) = &self.bwrap else {
            let m = "hooks configurados e nao ha bwrap para roda-los no sandbox".to_string();
            if ev == Evento::AntesDaFerramenta {
                s.bloqueio = Some(m);
            } else {
                s.avisos.push(m);
            }
            return s;
        };
        let texto = entrada_limitada(entrada);
        for c in lista {
            let cmd = WorkdirCommand {
                workdir: workdir.to_path_buf(),
                // O JSON entra pelo stdin, como no Claude Code; a variavel e so o meio.
                script: format!("printf '%s' \"$PHXCLAW_HOOK_INPUT\" | {{\n{}\n}}", c.linha),
                timeout: c.prazo,
                network: false,
                max_output_bytes: 16 * 1024,
            };
            let extras = SandboxExtras {
                ro_binds: vec![(self.pasta.clone(), "/hooks".into())],
                env: vec![
                    ("PHXCLAW_HOOK_INPUT".into(), texto.clone()),
                    ("PHXCLAW_HOOK_EVENT".into(), ev.nome().into()),
                    ("PHXCLAW_PROJECT_DIR".into(), "/hooks".into()),
                ],
            };
            match run_in_workdir_com(bwrap, &cmd, &extras) {
                Ok(r) if r.exit_code == Some(0) => {
                    let o = r.stdout.trim();
                    if !o.is_empty() {
                        s.contexto.push(o.to_string());
                    }
                }
                Ok(r) if r.exit_code == Some(2) => {
                    let m = r.stderr.trim();
                    s.bloqueio = Some(if m.is_empty() {
                        format!("hook {} saiu com 2 sem motivo", ev.nome())
                    } else {
                        m.to_string()
                    });
                    return s;
                }
                Ok(r) => s.avisos.push(format!(
                    "hook {} '{}' saiu com {:?}: {}",
                    ev.nome(),
                    c.linha,
                    r.exit_code,
                    r.stderr.trim()
                )),
                Err(e) => s
                    .avisos
                    .push(format!("hook {} '{}': {e}", ev.nome(), c.linha)),
            }
        }
        s
    }
}

fn casa(matcher: Option<&str>, ferramenta: Option<&str>) -> bool {
    match (matcher.map(str::trim), ferramenta) {
        (None | Some("*"), _) => true,
        (Some(m), Some(f)) => m.split('|').any(|x| x.trim() == f),
        (Some(_), None) => true,
    }
}

/// O JSON inteiro se couber; se nao, os campos grandes viram o tamanho deles. Cortar o
/// texto no meio entregaria ao hook um JSON quebrado.
fn entrada_limitada(v: &Value) -> String {
    let t = v.to_string();
    if t.len() <= ENTRADA_MAX {
        return t;
    }
    let mut v = v.clone();
    if let Some(o) = v.as_object_mut() {
        for campo in ["tool_input", "tool_response", "answer"] {
            if let Some(x) = o.get_mut(campo) {
                let n = x.to_string().len();
                if n > 4096 {
                    *x = json!({"_truncado": true, "bytes": n});
                }
            }
        }
    }
    v.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn le_o_formato_do_claude_code_e_recusa_evento_desconhecido() {
        let h = Hooks::de_json(
            r#"{"hooks":{"PreToolUse":[{"matcher":"shell|write_file","hooks":[{"type":"command","command":"x","timeout":5}]}],
                "Stop":[{"hooks":[{"command":"y"}]}]}}"#,
            PathBuf::from("/tmp"),
            None,
        )
        .unwrap();
        assert!(h.tem(Evento::AntesDaFerramenta) && h.tem(Evento::AntesDeResponder));
        assert!(!h.tem(Evento::FimDaTarefa));
        assert!(casa(Some("shell|write_file"), Some("shell")));
        assert!(!casa(Some("shell|write_file"), Some("read_file")));
        assert!(Hooks::de_json(r#"{"hooks":{"Quando":[]}}"#, PathBuf::new(), None).is_err());
    }

    #[test]
    fn sem_bwrap_a_guarda_fecha_e_os_outros_so_avisam() {
        let h = Hooks::de_json(
            r#"{"hooks":{"PreToolUse":[{"hooks":[{"command":"x"}]}],"TaskEnd":[{"hooks":[{"command":"x"}]}]}}"#,
            PathBuf::from("/tmp"),
            None,
        )
        .unwrap();
        let d = std::env::temp_dir();
        assert!(
            h.disparar(Evento::AntesDaFerramenta, Some("shell"), &d, &json!({}))
                .bloqueio
                .is_some()
        );
        let s = h.disparar(Evento::FimDaTarefa, None, &d, &json!({}));
        assert!(s.bloqueio.is_none() && !s.avisos.is_empty());
    }

    #[test]
    fn entrada_grande_continua_json_valido() {
        let v = json!({"tool_input": {"content": "x".repeat(200_000)}, "tool_name": "write_file"});
        let t = entrada_limitada(&v);
        assert!(t.len() < ENTRADA_MAX);
        let de_volta: Value = serde_json::from_str(&t).unwrap();
        assert_eq!(de_volta["tool_name"], "write_file");
        assert_eq!(de_volta["tool_input"]["_truncado"], true);
    }
}
