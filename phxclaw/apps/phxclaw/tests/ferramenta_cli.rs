//! `phxclaw ferramenta`, `phxclaw api` e `phxclaw completar` pelo processo de verdade.
//!
//! O que se prova aqui e o que um teste de unidade nao alcanca: a ferramenta chamada pela
//! CLI passa pelo PORTAO (capacidade negada recusa e nada se grava; a chamada aceita deixa
//! evidencia no ledger), e o `api` fala com um `servir` de pe, com o token que o servidor
//! gravou, e mostra o que o handler devolveu -- inclusive o 401.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

fn pasta(nome: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!(
        "phx-cli-ferramenta-{nome}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn cmd(dir: &Path) -> Command {
    let mut c = Command::new(env!("CARGO_BIN_EXE_phxclaw"));
    c.env_remove("PHXCLAW_HOME")
        .env_remove("PHXCLAW_CAPACIDADES")
        .env_remove("PHXCLAW_API_TOKEN")
        .env_remove("PHXCLAW_URL")
        .current_dir(dir);
    c
}

fn texto(o: &std::process::Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&o.stdout),
        String::from_utf8_lossy(&o.stderr)
    )
}

#[test]
fn ferramenta_passa_pelo_portao_e_deixa_evidencia() {
    let d = pasta("portao");
    let ag = d.join("agente");
    let o = cmd(&d)
        .args([
            "ferramenta",
            "write_file",
            "--path",
            "a.txt",
            "--content",
            "oi",
        ])
        .args(["--pasta", ag.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(o.status.success(), "{}", texto(&o));
    assert_eq!(std::fs::read_to_string(d.join("a.txt")).unwrap(), "oi");
    // A evidencia da chamada, no ledger da sessao que o stderr cita.
    let t = texto(&o);
    let caminho = t
        .lines()
        .find_map(|l| l.strip_prefix("ok; evidencia em "))
        .unwrap_or_else(|| panic!("sem caminho da evidencia: {t}"));
    let ev = std::fs::read_to_string(d.join(caminho)).unwrap();
    assert!(ev.contains("write_file"), "{ev}");

    // Capacidade negada: o portao recusa, sai 2, e o arquivo nao nasce.
    let o = cmd(&d)
        .env("PHXCLAW_CAPACIDADES", "fs.read")
        .args([
            "ferramenta",
            "write_file",
            "--path",
            "b.txt",
            "--content",
            "oi",
        ])
        .args(["--pasta", ag.to_str().unwrap()])
        .output()
        .unwrap();
    assert_eq!(o.status.code(), Some(2), "{}", texto(&o));
    assert!(texto(&o).contains("nao concedida"), "{}", texto(&o));
    assert!(!d.join("b.txt").exists(), "o atalho gravou sem o portao");

    // Argumento invalido pelo esquema: quem recusa e o validador do portao.
    let o = cmd(&d)
        .args(["ferramenta", "read_file", "--start_line", "1"])
        .args(["--pasta", ag.to_str().unwrap()])
        .output()
        .unwrap();
    assert_eq!(o.status.code(), Some(1), "{}", texto(&o));
    assert!(texto(&o).contains("path"), "{}", texto(&o));
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn a_ajuda_traz_os_parametros_dos_comandos_e_das_ferramentas() {
    let d = pasta("ajuda");
    let o = cmd(&d)
        .args(["ferramenta", "read_file", "--ajuda"])
        .output()
        .unwrap();
    assert!(o.status.success(), "{}", texto(&o));
    let t = texto(&o);
    assert!(t.contains("--path TEXTO  (obrigatorio)"), "{t}");
    assert!(t.contains("--start_line N"), "{t}");
    let o = cmd(&d).arg("ajuda").output().unwrap();
    let t = texto(&o);
    // Parametros, nao so nomes: o uso de cada comando vai na listagem geral.
    for p in ["--ponte-no", "--saida-esquema", "--consulta", "--falhar-em"] {
        assert!(t.contains(p), "ajuda sem {p}:\n{t}");
    }
    let o = cmd(&d).args(["servir", "--ajuda"]).output().unwrap();
    assert!(texto(&o).contains("--ponte-tenant"), "{}", texto(&o));
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn completar_bash_e_sintaxe_valida() {
    let d = pasta("completar");
    let o = cmd(&d).args(["completar", "bash"]).output().unwrap();
    assert!(o.status.success(), "{}", texto(&o));
    let arq = d.join("c.bash");
    std::fs::write(&arq, &o.stdout).unwrap();
    let b = Command::new("bash").arg("-n").arg(&arq).output().unwrap();
    assert!(b.status.success(), "{}", texto(&b));
    let o = cmd(&d).args(["completar", "tcsh"]).output().unwrap();
    assert!(!o.status.success());
    let _ = std::fs::remove_dir_all(&d);
}

/// Mata o filho no fim, passe ou caia o teste: servidor orfao seguraria a porta.
struct Filho(std::process::Child);
impl Drop for Filho {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[test]
fn api_chama_o_servidor_de_pe_com_o_token_local() {
    let d = pasta("api");
    let ag = d.join("agente");
    let porta = std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let mut srv = Filho(
        cmd(&d)
            .args(["servir", "--porta", &porta.to_string()])
            .args(["--pasta", ag.to_str().unwrap()])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap(),
    );
    let url = format!("http://127.0.0.1:{porta}");
    let api = |args: &[&str]| {
        cmd(&d)
            .arg("api")
            .args(args)
            .args(["--url", &url, "--pasta", ag.to_str().unwrap()])
            .output()
            .unwrap()
    };
    let inicio = Instant::now();
    loop {
        let o = api(&["GET", "/health"]);
        if o.status.success() {
            assert!(texto(&o).contains("\"ok\": true"), "{}", texto(&o));
            break;
        }
        if inicio.elapsed() > Duration::from_secs(60) {
            let mut e = String::new();
            let _ = srv.0.kill();
            srv.0.stderr.take().unwrap().read_to_string(&mut e).unwrap();
            panic!("servidor nao subiu: {}\n{e}", texto(&o));
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    let o = api(&["GET", "/v1/tasks"]);
    assert!(o.status.success(), "{}", texto(&o));
    assert!(texto(&o).contains("HTTP 200"), "{}", texto(&o));
    let o = api(&[
        "POST",
        "/v1/schedules",
        "--corpo",
        r#"{"name":"cli","objective":"oi","every_seconds":120}"#,
    ]);
    assert!(o.status.success(), "{}", texto(&o));
    let o = api(&["GET", "/v1/schedules"]);
    assert!(texto(&o).contains("\"name\": \"cli\""), "{}", texto(&o));
    // Recusa do handler sai com o codigo e o corpo dele.
    let o = cmd(&d)
        .env("PHXCLAW_API_TOKEN", "errado-errado-errado-errado-0")
        .args(["api", "GET", "/v1/tasks", "--url", &url])
        .args(["--pasta", ag.to_str().unwrap()])
        .output()
        .unwrap();
    assert_eq!(o.status.code(), Some(1), "{}", texto(&o));
    assert!(texto(&o).contains("HTTP 401"), "{}", texto(&o));
    // Rota fora da tabela nem sai do processo.
    let o = api(&["GET", "/v1/nada/de/nada"]);
    assert!(!o.status.success());
    assert!(texto(&o).contains("desconhecida"), "{}", texto(&o));
    drop(srv);
    let _ = std::fs::remove_dir_all(&d);
}
