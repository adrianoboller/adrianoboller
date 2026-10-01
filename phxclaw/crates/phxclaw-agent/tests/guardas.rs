//! Guardas da montagem e do portao: catraca das capacidades, nome repetido, corte do
//! subagente, regra de comando para quem cria processo sem declarar a linha, e a pétrea do
//! bubblewrap (todo processo criado nesta crate passa pelo sandbox ou esta numa lista de
//! excecoes DECLARADA, com o motivo).

use phxclaw_agent::montagem::{CAPACIDADES_PADRAO, Montagem};
use phxclaw_agent::motor::{CAPACIDADES_DE_LEITURA, CAPACIDADES_QUE_ESCREVEM};
use phxclaw_agent::regras::RegrasDeComando;
use phxclaw_agent::*;
use phxclaw_agent_core::{BoxFut, Tool, ToolContext, ToolError, ToolOutput, ToolSpec};
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

fn tmp(nome: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!(
        "phx-guarda-{nome}-{}",
        phxclaw_types::new_uuid_v7()
    ));
    std::fs::create_dir_all(&d).unwrap();
    d
}

struct Falsa {
    nome: &'static str,
    cap: &'static str,
    rodou: Arc<AtomicUsize>,
}

impl Falsa {
    fn nova(nome: &'static str, cap: &'static str) -> (Arc<Self>, Arc<AtomicUsize>) {
        let r = Arc::new(AtomicUsize::new(0));
        (
            Arc::new(Self {
                nome,
                cap,
                rodou: r.clone(),
            }),
            r,
        )
    }
}

impl Tool for Falsa {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: self.nome.into(),
            description: "falsa".into(),
            parameters: json!({"type":"object"}),
        }
    }
    fn capability(&self) -> &'static str {
        self.cap
    }
    fn run<'a>(
        &'a self,
        _a: Value,
        _c: &'a ToolContext,
    ) -> BoxFut<'a, Result<ToolOutput, ToolError>> {
        Box::pin(async move {
            self.rodou.fetch_add(1, Ordering::SeqCst);
            Ok(ToolOutput::text(format!("rodou {}", self.cap)))
        })
    }
}

// ------------------------------------------------------------------ catraca das capacidades

/// Toda string `a.b` devolvida por um `fn capability` no fonte da crate: pega tambem as
/// ferramentas que esta maquina nao monta (sem chromium, sem SMTP, sem feature desktop).
fn capacidades_no_fonte() -> BTreeSet<String> {
    let mut v = BTreeSet::new();
    for arq in arquivos_rs(&Path::new(env!("CARGO_MANIFEST_DIR")).join("src")) {
        let t = std::fs::read_to_string(&arq).unwrap();
        for (i, _) in t.match_indices("fn capability(&self)") {
            let corpo = &t[i..];
            // O metodo acaba no primeiro `}` na indentacao dele.
            let fim = corpo.find("\n    }").unwrap_or(corpo.len());
            for (k, s) in corpo[..fim].split('"').enumerate() {
                let forma = s.split_once('.').is_some_and(|(a, b)| {
                    !a.is_empty()
                        && !b.is_empty()
                        && s.chars()
                            .all(|c| c.is_ascii_lowercase() || c == '.' || c == '_')
                });
                if k % 2 == 1 && forma {
                    v.insert(s.to_string());
                }
            }
        }
    }
    v
}

fn arquivos_rs(d: &Path) -> Vec<PathBuf> {
    let mut v = Vec::new();
    for e in std::fs::read_dir(d).unwrap().flatten() {
        let p = e.path();
        if p.is_dir() {
            v.extend(arquivos_rs(&p));
        } else if p.extension().is_some_and(|x| x == "rs") {
            v.push(p);
        }
    }
    v.sort();
    v
}

#[test]
fn catraca_das_capacidades_padrao_e_da_classificacao() {
    // O conjunto exato do padrao: capacidade nova no padrao e decisao de politica, e entra
    // mudando ESTA lista no mesmo passo, com o motivo no comentario do padrao.
    let padrao: BTreeSet<&str> = CAPACIDADES_PADRAO.iter().copied().collect();
    let esperado: BTreeSet<&str> = [
        "web.search",
        "web.browse",
        "fs.read",
        "fs.write",
        "doc.write",
        "shell.exec",
        "agent.spawn",
        "agent.parallel",
        "site.publish",
        "user.ask",
        "session.read",
        "calc",
        "memory.read",
        "memory.write",
        "skill.read",
        "doc.read",
        "web.research",
        "team.read",
        "team.delegate",
        "git.read",
        "git.write",
        "code.review",
        "device.read",
    ]
    .into_iter()
    .collect();
    assert_eq!(padrao, esperado, "CAPACIDADES_PADRAO mudou sem a catraca");

    let leitura: BTreeSet<&str> = CAPACIDADES_DE_LEITURA.iter().copied().collect();
    let escrita: BTreeSet<&str> = CAPACIDADES_QUE_ESCREVEM.iter().copied().collect();
    let ambas: Vec<_> = leitura.intersection(&escrita).collect();
    assert!(ambas.is_empty(), "capacidade nas duas listas: {ambas:?}");

    let mut conhecidas = capacidades_no_fonte();
    let m = Montagem::new(TaskStore::new(tmp("caps")).unwrap());
    let a = m.agent_with(Arc::new(ScriptedLlm::new(vec![])));
    conhecidas.extend(a.tools.iter().map(|t| t.capability().to_string()));
    conhecidas.extend(CAPACIDADES_PADRAO.iter().map(|s| s.to_string()));
    assert!(
        conhecidas.len() >= 30,
        "o varredor do fonte parou de achar: {conhecidas:?}"
    );
    let soltas: Vec<_> = conhecidas
        .iter()
        .filter(|c| !c.starts_with("mcp."))
        .filter(|c| !leitura.contains(c.as_str()) && !escrita.contains(c.as_str()))
        .collect();
    assert!(
        soltas.is_empty(),
        "capacidade sem classificacao (CAPACIDADES_DE_LEITURA ou CAPACIDADES_QUE_ESCREVEM): {soltas:?}"
    );
}

// ------------------------------------------------------------------ nome repetido

#[tokio::test]
async fn nome_repetido_se_recusa_na_montagem_e_no_portao() {
    // MCP de fora com nome de ferramenta nativa: fica de fora, a nativa vale.
    let mut m = Montagem::new(TaskStore::new(tmp("dup")).unwrap());
    let (falsa, rodou) = Falsa::nova("write_file", "mcp.fora");
    m.mcp = vec![falsa];
    let a = m.agent_with(Arc::new(ScriptedLlm::new(vec![])));
    let w: Vec<_> = a
        .tools
        .iter()
        .filter(|t| t.spec().name == "write_file")
        .collect();
    assert_eq!(w.len(), 1);
    assert_eq!(w[0].capability(), "fs.write");

    // Repetida entre as que nao sao MCP: a montagem recusa.
    let mut m = Montagem::new(TaskStore::new(tmp("dup2")).unwrap());
    let (outra, _) = Falsa::nova("read_file", "fs.write");
    m.forjas = vec![outra];
    let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        m.agent_with(Arc::new(ScriptedLlm::new(vec![])))
    }));
    let e = r.err().expect("a montagem aceitou nome repetido");
    let msg = e.downcast_ref::<String>().cloned().unwrap_or_default();
    assert!(msg.contains("repetido: read_file"), "{msg}");

    // Agent montado a mao com nome repetido: o portao nao escolhe por ordem.
    let (inofensiva, r1) = Falsa::nova("acao", "fs.read");
    let (perigosa, r2) = Falsa::nova("acao", "shell.exec");
    let llm = Arc::new(ScriptedLlm::new(vec![
        ScriptedLlm::call("c1", "acao", json!({})),
        ScriptedLlm::text("fim"),
    ]));
    let ag = Agent::new(
        llm,
        vec![inofensiva, perigosa],
        AgentConfig::default().grant(&["fs.read"]),
        TaskStore::new(tmp("dup3")).unwrap(),
    );
    let t = ag
        .run(Task::new("x", "m"), &CancelFlag::default(), &NoObserver)
        .await;
    assert_eq!(t.steps[0].outcome, "negado", "{:?}", t.steps[0]);
    assert!(t.steps[0].summary.contains("repetido"));
    assert_eq!(r1.load(Ordering::SeqCst) + r2.load(Ordering::SeqCst), 0);
    assert_eq!(rodou.load(Ordering::SeqCst), 0);
}

// ------------------------------------------------------------------ corte do subagente

#[tokio::test]
async fn filho_do_parallel_research_nao_ve_delegar_nem_equipe() {
    let (spawn, _) = Falsa::nova("outro_spawn", "agent.spawn");
    let (equipe, _) = Falsa::nova("team_list", "team.read");
    let (delega, _) = Falsa::nova("team_delegate", "team.delegate");
    let llm = Arc::new(ScriptedLlm::new(vec![ScriptedLlm::text(
        "resposta do filho",
    )]));
    let tools: Vec<Arc<dyn Tool>> = vec![spawn, equipe, delega, Arc::new(ListFilesTool)];
    let pr = ParallelAgentsTool {
        llm: llm.clone(),
        tools,
        config: AgentConfig::default().grant(&[
            "fs.read",
            "agent.spawn",
            "team.read",
            "team.delegate",
        ]),
        store: TaskStore::new(tmp("par")).unwrap(),
        max_parallel: 2,
    };
    let c = ToolContext {
        task_id: "mae".into(),
        workdir: tmp("par-w"),
        timeout: std::time::Duration::from_secs(10),
    };
    let o = pr.run(json!({"subtasks": ["a"]}), &c).await.unwrap();
    assert!(o.content.contains("resposta do filho"), "{}", o.content);
    let vistas = llm.seen.lock().unwrap()[0].1.clone();
    assert_eq!(
        vistas,
        vec!["list_files".to_string()],
        "o filho viu: {vistas:?}"
    );
}

// ------------------------------------------------------------------ regra sem linha declarada

#[tokio::test]
async fn regra_alcanca_ferramenta_que_cria_processo_sem_declarar_a_linha() {
    let (git, rodou) = Falsa::nova("git_write", "git.write");
    let regras = RegrasDeComando::de_json(
        r#"{"padrao":"negar","regras":[{"padrao":"git_write status","decisao":"permitir"}]}"#,
    )
    .unwrap();
    let llm = Arc::new(ScriptedLlm::new(vec![
        ScriptedLlm::call("c1", "git_write", json!({"action": "push"})),
        ScriptedLlm::call("c2", "git_write", json!({"action": "status"})),
        ScriptedLlm::call("c3", "list_files", json!({})),
        ScriptedLlm::text("fim"),
    ]));
    let ag = Agent::new(
        llm,
        vec![git, Arc::new(ListFilesTool)],
        AgentConfig {
            regras: Some(Arc::new(regras)),
            ..AgentConfig::default()
        }
        .grant(&["git.write", "fs.read"]),
        TaskStore::new(tmp("reg")).unwrap(),
    );
    let t = ag
        .run(Task::new("x", "m"), &CancelFlag::default(), &NoObserver)
        .await;
    assert_eq!(t.steps[0].outcome, "negado", "{:?}", t.steps[0]);
    assert!(t.steps[0].summary.contains("git_write"), "{:?}", t.steps[0]);
    assert_eq!(t.steps[1].outcome, "ok");
    assert_eq!(rodou.load(Ordering::SeqCst), 1, "so o status rodou");
    // Ler arquivo nao cria processo: regra de COMANDO nao se aplica a ela.
    assert_eq!(t.steps[2].outcome, "ok", "{:?}", t.steps[2]);
}

// ------------------------------------------------------------------ pétrea do bubblewrap

/// Processos criados FORA do bwrap nesta crate, cada um com o motivo. Entrada nova aqui e
/// decisao, nao conveniencia: o motivo tem de dizer por que o sandbox nao serve.
const FORA_DO_BWRAP: &[(&str, &str, &str)] = &[
    (
        "sistema.rs",
        "Command::new(&prog)",
        "linux_system/network existem para ver e mudar o HOSPEDEIRO (systemctl, ip, ping); no \
sandbox nao veriam nada. Binario por caminho fixo, env_clear, capacidades fora do padrao.",
    ),
    (
        "sistema.rs",
        "Command::new(rustc)",
        "sondagem do toolchain na montagem (`rustc --print sysroot`), sem entrada do modelo; \
env_clear com so as variaveis do rustup. O cargo da tarefa roda no bwrap.",
    ),
    (
        "python.rs",
        "Command::new(caminho)",
        "sondagem do interpretador na montagem (`-I`, env_clear), sem entrada do modelo; o \
codigo da tarefa roda no bwrap.",
    ),
    (
        "python.rs",
        "Command::new(\"/bin/sh\")",
        "teste unitario das aspas contra o shell de verdade (#[cfg(test)]).",
    ),
    (
        "visao.rs",
        "Command::new(&chromium)",
        "o Chromium tem sandbox proprio (namespaces, seccomp) que nao sobe dentro do bwrap, e \
precisa de /dev/shm e fontes; a pagina so alcanca a pasta pelo proxy loopback, sem rede. \
env_clear.",
    ),
    (
        "visao.rs",
        "transcribe_verified(",
        "whisper.cpp nasce na crate phxclaw-media-intelligence, com o modelo conferido por \
SHA-256 antes de rodar; capacidade media.stt fora do padrao. DIVIDA: levar ao bwrap.",
    ),
    (
        "avaliacao.rs",
        "Command::new(nvidia)",
        "leitura do contador de energia da GPU (`nvidia-smi --query-gpu=total_energy_consumption`) \
pelo `phxclaw avaliar`: precisa do /dev/nvidia* do hospedeiro, que o bwrap nao expoe. Argumentos \
fixos, sem entrada do modelo, env_clear; so roda se o binario existir no PATH.",
    ),
];

/// Maneiras de criar processo que a guarda procura. `transcribe_verified` e porta
/// indireta: o processo nasce noutra crate, mas a decisao de chama-lo e daqui.
const PORTAS: &[&str] = &[
    "Command::new(",
    "process::Command as",
    "transcribe_verified(",
];

#[test]
fn todo_processo_desta_crate_passa_pelo_bwrap_ou_esta_declarado() {
    let raiz = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut usadas = vec![false; FORA_DO_BWRAP.len()];
    let mut soltos = Vec::new();
    for arq in arquivos_rs(&raiz) {
        let rel = arq
            .strip_prefix(&raiz)
            .unwrap()
            .to_string_lossy()
            .into_owned();
        for (n, linha) in std::fs::read_to_string(&arq).unwrap().lines().enumerate() {
            let l = linha.trim_start();
            if l.starts_with("//") || !PORTAS.iter().any(|p| l.contains(p)) {
                continue;
            }
            match FORA_DO_BWRAP
                .iter()
                .position(|(a, trecho, _)| *a == rel && l.contains(trecho))
            {
                Some(i) => usadas[i] = true,
                None => soltos.push(format!("{rel}:{}: {}", n + 1, l)),
            }
        }
    }
    assert!(
        soltos.is_empty(),
        "processo fora do bwrap sem excecao declarada em FORA_DO_BWRAP:\n{}",
        soltos.join("\n")
    );
    let mortas: Vec<_> = FORA_DO_BWRAP
        .iter()
        .zip(&usadas)
        .filter(|(_, u)| !**u)
        .map(|((a, t, _), _)| format!("{a}: {t}"))
        .collect();
    assert!(
        mortas.is_empty(),
        "excecao que nao existe mais no fonte: {mortas:?}"
    );
    assert!(
        FORA_DO_BWRAP.iter().all(|(_, _, m)| m.len() > 40),
        "excecao sem motivo"
    );
}
