//! Os comandos do terminal do IDE: a ponte fina entre a WebView e o `phxclaw-terminal`.
//!
//! Aqui so se decide QUEM pode abrir o que; PTY, emulador, tecla e diferenca de grade
//! moram no crate, para que tela, teste e agente usem o mesmo motor.

use crate::{DesktopState, record_simple_evidence};
use phxclaw_evidence_ledger::EvidenceOutcome;
use phxclaw_terminal::{Atualizacao, Programa, Tamanho, Tecla, Terminal};
use phxclaw_types::new_uuid_v7;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, State};

/// Nome do evento que leva a grade (so o que mudou) para a tela.
pub const EVENTO_GRADE: &str = "terminal_grade";

/// Teto de terminais vivos: cada um e um processo e um fio de leitura. Uma tela com
/// defeito abrindo em laco esgotaria os PTYs da maquina inteira.
const MAXIMO_DE_TERMINAIS: usize = 16;

#[derive(Default)]
pub struct Terminais(Mutex<HashMap<String, Terminal>>);

impl Terminais {
    /// Solta todos, matando os filhos. Chamado na saida do aplicativo: o Tauri encerra o
    /// processo sem soltar o estado gerenciado.
    pub fn fechar_todos(&self) {
        let todos: Vec<Terminal> = match self.0.lock() {
            Ok(mut m) => m.drain().map(|(_, t)| t).collect(),
            Err(_) => return,
        };
        drop(todos);
    }
}

/// So dois programas, por nome: a tela nao escolhe executavel. Um caminho livre aqui seria
/// o `execute_shell` sem a politica dele.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ProgramaTerminal {
    Bash,
    Helix,
}

#[derive(Debug, Clone, Serialize)]
pub struct TerminalAberto {
    pub id: String,
    pub pid: u32,
    pub programa: ProgramaTerminal,
    pub cwd: PathBuf,
}

#[derive(Debug, Clone, Serialize)]
struct EventoGrade<'a> {
    id: &'a str,
    #[serde(flatten)]
    grade: &'a Atualizacao,
}

/// Pasta que o IDE abre: `PHXCLAW_PROJETO`, ou a pasta de onde o aplicativo subiu.
pub fn pasta_do_projeto() -> PathBuf {
    std::env::var_os("PHXCLAW_PROJETO")
        .map(PathBuf::from)
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_else(|| PathBuf::from("."))
}

/// O hx instalado pelo `tools/instalar_helix.sh` fica em /opt/helix com o runtime ao lado;
/// `PHXCLAW_HX` aponta outro. Sem nenhum dos dois, o do PATH.
fn achar_helix() -> Option<(PathBuf, Option<PathBuf>)> {
    if let Some(p) = std::env::var_os("PHXCLAW_HX").map(PathBuf::from) {
        return p.is_file().then_some((p, None));
    }
    let opt = Path::new("/opt/helix/hx");
    if opt.is_file() {
        let runtime = Path::new("/opt/helix/runtime");
        return Some((opt.into(), runtime.is_dir().then(|| runtime.into())));
    }
    std::env::var_os("PATH").and_then(|path| {
        std::env::split_paths(&path)
            .map(|d| d.join("hx"))
            .find(|p| p.is_file())
            .map(|p| (p, None))
    })
}

fn montar(programa: ProgramaTerminal, cwd: &Path) -> Result<Programa, String> {
    match programa {
        ProgramaTerminal::Bash => Ok(Programa {
            programa: "bash".into(),
            args: vec!["-l".into()],
            cwd: Some(cwd.into()),
            env: vec![],
        }),
        ProgramaTerminal::Helix => {
            let (hx, runtime) = achar_helix().ok_or(
                "hx nao encontrado: rode phxclaw/tools/instalar_helix.sh (instala em /opt/helix) \
                 ou aponte PHXCLAW_HX para o executavel",
            )?;
            let mut env = vec![];
            if let Some(r) = runtime {
                env.push(("HELIX_RUNTIME".into(), r.display().to_string()));
            }
            // Workspace de varias raizes: o hx abre a PRIMEIRA (a pasta do projeto) e as
            // outras vao na variavel (`ide.raizes` do catalogo), para o shell e o proprio
            // usuario as acharem sem segunda janela. Arquivo invalido e erro dito.
            let raizes = phxclaw_workspace::raizes(&cwd.join(".phxclaw"))?;
            if !raizes.is_empty() {
                let lista = phxclaw_workspace::variavel(&raizes);
                env.push(("PHXCLAW_RAIZES".into(), lista));
            }
            Ok(Programa {
                programa: hx.display().to_string(),
                args: vec![".".into()],
                cwd: Some(cwd.into()),
                env,
            })
        }
    }
}

/// O terminal e de quem digita. Se a WebView pode ser dirigida de fora (webview_control),
/// quem a dirige alcancaria o shell pelo terminal; entao, nesse caso, o terminal pede a
/// mesma permissao do `execute_shell`.
pub fn permitido(command_execution: bool, webview_control: bool) -> bool {
    command_execution || !webview_control
}

fn erro(e: impl std::fmt::Display) -> String {
    e.to_string()
}

#[tauri::command]
pub fn terminal_abrir(
    app: AppHandle,
    host: State<'_, DesktopState>,
    terminais: State<'_, Terminais>,
    programa: ProgramaTerminal,
    colunas: u16,
    linhas: u16,
) -> Result<TerminalAberto, String> {
    let cwd = pasta_do_projeto();
    if !host.policy.interactive_terminal {
        let _ = record_simple_evidence(
            &host,
            new_uuid_v7(),
            "command-center",
            "terminal.open",
            "terminal_abrir",
            EvidenceOutcome::Denied,
            json!({"programa": programa}),
            json!({"motivo": "webview_control ligado sem command_execution"}),
            vec![],
        );
        return Err(
            "terminal negado pela politica: com PHXCLAW_ENABLE_WEBVIEW_CONTROL ligado, o \
             terminal exige PHXCLAW_ENABLE_HOST_EXEC"
                .into(),
        );
    }
    let mut mapa = terminais.0.lock().map_err(|_| "estado dos terminais")?;
    if mapa.len() >= MAXIMO_DE_TERMINAIS {
        return Err(format!(
            "ja ha {MAXIMO_DE_TERMINAIS} terminais abertos; feche um antes"
        ));
    }
    let id = new_uuid_v7().to_string();
    let emissor = app.clone();
    let id_evento = id.clone();
    let t = Terminal::abrir(
        montar(programa, &cwd)?,
        Tamanho { colunas, linhas },
        move |grade| {
            let _ = emissor.emit(
                EVENTO_GRADE,
                EventoGrade {
                    id: &id_evento,
                    grade: &grade,
                },
            );
        },
    )
    .map_err(erro)?;
    let aberto = TerminalAberto {
        id: id.clone(),
        pid: t.pid(),
        programa,
        cwd: cwd.clone(),
    };
    mapa.insert(id.clone(), t);
    drop(mapa);
    let _ = record_simple_evidence(
        &host,
        new_uuid_v7(),
        "command-center",
        "terminal.open",
        "terminal_abrir",
        EvidenceOutcome::Succeeded,
        json!({"programa": programa, "cwd": cwd}),
        json!({"id": id, "pid": aberto.pid}),
        vec![],
    );
    Ok(aberto)
}

/// Um dos tres: `tecla` (do teclado, traduzida pelo modo do terminal), `colar` (texto
/// colado, delimitado se o programa pediu) ou `texto` (bytes crus, para atalho da tela).
#[tauri::command]
pub fn terminal_escrever(
    terminais: State<'_, Terminais>,
    id: String,
    tecla: Option<Tecla>,
    colar: Option<String>,
    texto: Option<String>,
) -> Result<bool, String> {
    let mapa = terminais.0.lock().map_err(|_| "estado dos terminais")?;
    let t = mapa.get(&id).ok_or("terminal desconhecido")?;
    match (tecla, colar, texto) {
        (Some(k), None, None) => t.tecla(&k).map_err(erro),
        (None, Some(c), None) => t.colar(&c).map(|_| true).map_err(erro),
        (None, None, Some(s)) => t.escrever(s.as_bytes()).map(|_| true).map_err(erro),
        _ => Err("mande exatamente um de: tecla, colar, texto".into()),
    }
}

#[tauri::command]
pub fn terminal_redimensionar(
    terminais: State<'_, Terminais>,
    id: String,
    colunas: u16,
    linhas: u16,
) -> Result<(), String> {
    let mapa = terminais.0.lock().map_err(|_| "estado dos terminais")?;
    let t = mapa.get(&id).ok_or("terminal desconhecido")?;
    t.redimensionar(Tamanho { colunas, linhas }).map_err(erro)
}

#[tauri::command]
pub fn terminal_rolar(
    terminais: State<'_, Terminais>,
    id: String,
    linhas: i32,
) -> Result<(), String> {
    let mapa = terminais.0.lock().map_err(|_| "estado dos terminais")?;
    mapa.get(&id).ok_or("terminal desconhecido")?.rolar(linhas);
    Ok(())
}

#[tauri::command]
pub fn terminal_fechar(
    host: State<'_, DesktopState>,
    terminais: State<'_, Terminais>,
    id: String,
) -> Result<(), String> {
    // Tira do mapa e solta FORA da trava: soltar espera o filho morrer (ate meio segundo
    // para um filho surdo ao SIGHUP), e os outros terminais nao podem parar por isso.
    let t = terminais
        .0
        .lock()
        .map_err(|_| "estado dos terminais")?
        .remove(&id)
        .ok_or("terminal desconhecido")?;
    let pid = t.pid();
    drop(t);
    let _ = record_simple_evidence(
        &host,
        new_uuid_v7(),
        "command-center",
        "terminal.close",
        "terminal_fechar",
        EvidenceOutcome::Succeeded,
        json!({"id": id}),
        json!({"pid": pid}),
        vec![],
    );
    Ok(())
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn webview_dirigivel_so_tem_terminal_com_permissao_de_comando() {
        // Padrao (nada ligado): uma pessoa no IDE tem terminal.
        assert!(permitido(false, false));
        // A tela pode ser dirigida e o comando esta negado: o terminal seria a porta dos
        // fundos do execute_shell.
        assert!(!permitido(false, true));
        assert!(permitido(true, true));
        assert!(permitido(true, false));
    }

    #[test]
    fn so_dois_programas_por_nome() {
        let cwd = Path::new("/tmp");
        let b = montar(ProgramaTerminal::Bash, cwd).unwrap();
        assert_eq!(b.programa, "bash");
        assert_eq!(b.cwd.as_deref(), Some(cwd));
        // Nome de programa que nao e um dos dois nao desserializa: a tela nao escolhe binario.
        assert!(serde_json::from_str::<ProgramaTerminal>("\"sh\"").is_err());
        assert_eq!(
            serde_json::from_str::<ProgramaTerminal>("\"helix\"").unwrap(),
            ProgramaTerminal::Helix
        );
    }
}
