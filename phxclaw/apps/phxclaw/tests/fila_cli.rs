//! `phxclaw servir --modo fila` + `phxclaw worker` pelos PROCESSOS de verdade, contra um
//! PostgreSQL de verdade: o worker e morto com SIGKILL no meio de um passo, e outro worker
//! retoma a execucao depois do prazo -- o passo que ja tinha terminado nao roda de novo. A
//! senha do banco chega pelo broker (`phxclaw fila senha`), nunca pela URL do config.json.
//!
//! O teste do agente (`crates/phxclaw-agent/tests/fila_workers.rs`) prova as regras; este
//! prova que a CLI e porta fina para elas e que a queda e a do sistema operacional.

use phxclaw_test_support::pg::PgEfemero;
use phxclaw_test_support::pulado;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

fn pasta(nome: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!(
        "phx-cli-fila-{nome}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn phxclaw(dir: &Path) -> Command {
    let mut c = Command::new(env!("CARGO_BIN_EXE_phxclaw"));
    c.env_remove("PHXCLAW_HOME")
        .env_remove("PHXCLAW_FILA_URL")
        .env_remove("PHXCLAW_FILA_SENHA")
        .current_dir(dir);
    c
}

/// Processo filho morto no `Drop` (inclusive quando o teste falha no meio).
struct Filho(Child, mpsc::Receiver<String>);

impl Filho {
    fn subir(mut c: Command) -> Self {
        let mut ch = c
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let (tx, rx) = mpsc::channel();
        for leitor in [
            Box::new(ch.stdout.take().unwrap()) as Box<dyn Read + Send>,
            Box::new(ch.stderr.take().unwrap()),
        ] {
            let tx = tx.clone();
            std::thread::spawn(move || {
                for l in BufReader::new(leitor).lines().map_while(Result::ok) {
                    let _ = tx.send(l);
                }
            });
        }
        Self(ch, rx)
    }

    /// Espera uma linha que contem `t`; devolve todas as linhas lidas ate ela.
    fn ate(&self, t: &str, limite: Duration) -> Vec<String> {
        let fim = Instant::now() + limite;
        let mut v = vec![];
        while Instant::now() < fim {
            if let Ok(l) = self.1.recv_timeout(Duration::from_millis(100)) {
                let achou = l.contains(t);
                v.push(l);
                if achou {
                    return v;
                }
            }
        }
        panic!("sem a linha {t:?} em {limite:?}: {v:#?}");
    }
}

impl Drop for Filho {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn http(porta: u16, metodo: &str, caminho: &str, token: &str, corpo: &str) -> String {
    let mut s = std::net::TcpStream::connect(("127.0.0.1", porta)).unwrap();
    write!(
        s,
        "{metodo} {caminho} HTTP/1.1\r\nHost: 127.0.0.1\r\nAuthorization: Bearer {token}\r\n\
Content-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{corpo}",
        corpo.len()
    )
    .unwrap();
    let mut r = String::new();
    s.read_to_string(&mut r).unwrap();
    r
}

fn tarefa(raiz: &Path, id: &str) -> serde_json::Value {
    std::fs::read_to_string(raiz.join("tasks").join(id).join("task.json"))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

#[test]
fn worker_morto_com_sigkill_outro_worker_retoma_pela_cli() {
    let pg = match PgEfemero::subir() {
        Ok(pg) => pg,
        Err(e) => {
            pulado::pular("initdb", &e);
            return;
        }
    };
    pg.aplicar(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../migrations/0004_task_graph.sql"))
        .unwrap();
    let d = pasta("sigkill");
    let raiz = d.join("agente");
    std::fs::create_dir_all(raiz.join("fluxos")).unwrap();
    // A URL do operador SEM a senha; a senha vai para o broker pelo comando.
    std::fs::write(
        raiz.join("config.json"),
        serde_json::json!({"fila": {"url": pg.url_sem_senha(), "prazo_segundos": 3}}).to_string(),
    )
    .unwrap();
    let o = phxclaw(&d)
        .args(["fila", "senha", "--pasta", raiz.to_str().unwrap()])
        .env("PHXCLAW_FILA_SENHA", &pg.senha)
        .output()
        .unwrap();
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    assert!(!String::from_utf8_lossy(&o.stdout).contains(&pg.senha));
    std::fs::write(
        raiz.join("fluxos/lento.json"),
        r#"{"nome":"lento","passos":[
            {"id":"a","ferramenta":"shell","args":{"command":"echo um >> conta.txt"}},
            {"id":"b","depende":["a"],"ferramenta":"shell","args":{"command":"sleep 6; echo fim > fim.txt"}}]}"#,
    )
    .unwrap();
    let porta = std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let mut c = phxclaw(&d);
    c.args([
        "servir",
        "--modo",
        "fila",
        "--porta",
        &porta.to_string(),
        "--pasta",
        raiz.to_str().unwrap(),
    ]);
    let servir = Filho::subir(c);
    let linhas = servir.ate("API de tarefas em", Duration::from_secs(60));
    assert!(
        linhas.iter().any(|l| l.contains("modo fila")),
        "{linhas:#?}"
    );
    let token = std::fs::read_to_string(raiz.join("api.token")).unwrap();
    let r = http(
        porta,
        "POST",
        "/v1/fluxos/rodar",
        token.trim(),
        r#"{"nome":"lento.json"}"#,
    );
    assert!(r.starts_with("HTTP/1.1 202"), "{r}");
    let corpo = &r[r.find("\r\n\r\n").unwrap() + 4..];
    let id = serde_json::from_str::<serde_json::Value>(corpo.trim())
        .ok()
        .and_then(|v| v["id"].as_str().map(str::to_string))
        .unwrap_or_else(|| panic!("{r}"));
    // Ninguem roda no servir: sem worker, a execucao fica na fila.
    std::thread::sleep(Duration::from_millis(800));
    assert_eq!(
        tarefa(&raiz, &id)["status"],
        "pending",
        "o servir em modo fila executou"
    );
    let worker = |nome: &str| {
        let mut c = phxclaw(&d);
        c.args([
            "worker",
            "--pasta",
            raiz.to_str().unwrap(),
            "--nome",
            nome,
            "--concorrencia",
            "1",
        ]);
        Filho::subir(c)
    };
    let mut w1 = worker("w1");
    let conta = raiz.join("tasks").join(&id).join("work/conta.txt");
    let fim = Instant::now() + Duration::from_secs(60);
    while !conta.exists() && Instant::now() < fim {
        std::thread::sleep(Duration::from_millis(100));
    }
    assert!(conta.exists(), "o worker 1 nao rodou o passo a");
    std::thread::sleep(Duration::from_millis(1500));
    // SIGKILL: nada fecha o run, nada bate mais.
    w1.0.kill().unwrap();
    let _ = w1.0.wait();
    let w2 = worker("w2");
    let linhas = w2.ate(&id, Duration::from_secs(60));
    let ultima = linhas.last().unwrap().clone();
    assert!(
        ultima.contains("(retomada)") && ultima.contains("Completed"),
        "{linhas:#?}"
    );
    let t = tarefa(&raiz, &id);
    assert_eq!(t["status"], "completed", "{t:#}");
    assert_eq!(
        std::fs::read_to_string(&conta).unwrap().lines().count(),
        1,
        "o passo a rodou de novo depois da queda"
    );
    let runs = pg
        .sql(&format!(
            "SELECT r.attempt || ':' || r.status FROM phoenix_task_runs r JOIN phoenix_tasks t ON t.uuid = r.task_uuid \
             WHERE t.payload->>'tarefa' = '{id}' ORDER BY r.attempt"
        ))
        .unwrap();
    assert_eq!(runs, "1:failed\n2:succeeded", "{runs}");
    // O servir le o fim pelo disco compartilhado: a API responde o que o worker gravou.
    let r = http(porta, "GET", &format!("/v1/tasks/{id}"), token.trim(), "");
    assert!(r.contains("\"completed\""), "{r}");
    drop((w2, servir));
    let _ = std::fs::remove_dir_all(&d);
}
