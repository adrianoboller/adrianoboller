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

// ------------------------------------------------------------- A3: para onde o token vai

/// Servidor falso: conta conexoes e cabecalhos `Authorization` recebidos, e responde
/// `{"ok":true}`. O `0.0.0.0:PORTA` chega a ele pelo kernel, e e um host que o `api` NAO
/// trata como loopback -- e assim que o teste ve, sem rede, o que iria «para fora».
struct Falso {
    porta: u16,
    conexoes: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    autorizacoes: std::sync::Arc<std::sync::atomic::AtomicUsize>,
}

fn falso() -> Falso {
    use std::io::Write;
    use std::sync::atomic::Ordering;
    let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let porta = l.local_addr().unwrap().port();
    let conexoes = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let autorizacoes = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let (c, a) = (conexoes.clone(), autorizacoes.clone());
    std::thread::spawn(move || {
        for s in l.incoming().flatten() {
            c.fetch_add(1, Ordering::SeqCst);
            let mut s = s;
            let _ = s.set_read_timeout(Some(Duration::from_secs(5)));
            let mut lido = Vec::new();
            let mut buf = [0u8; 4096];
            while !lido.windows(4).any(|w| w == b"\r\n\r\n") {
                match s.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => lido.extend_from_slice(&buf[..n]),
                }
            }
            if String::from_utf8_lossy(&lido)
                .to_ascii_lowercase()
                .contains("\r\nauthorization:")
            {
                a.fetch_add(1, Ordering::SeqCst);
            }
            let _ = s.write_all(
                b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 11\r\n\
Connection: close\r\n\r\n{\"ok\":true}",
            );
        }
    });
    Falso {
        porta,
        conexoes,
        autorizacoes,
    }
}

impl Falso {
    fn contas(&self) -> (usize, usize) {
        use std::sync::atomic::Ordering;
        (
            self.conexoes.load(Ordering::SeqCst),
            self.autorizacoes.load(Ordering::SeqCst),
        )
    }
}

const TOKEN_TESTE: &str = "token-do-teste-com-mais-de-24-caracteres";

/// A3 (09/10/2026), pelo processo de verdade: um projeto CONFIADO cujo
/// `.phxclaw/config.json` aponta `api.url_cliente` para fora nao leva o Bearer -- vale a
/// url do operador (a pasta), e o aviso diz a chave ignorada. E `--url` http fora de
/// loopback e recusado ANTES de qualquer conexao, com ou sem `--confio-nesta-url`; https de
/// fora so com o interruptor. O servidor falso conta zero conexoes em toda recusa.
///
/// RED medido, cada um com o defeito reposto e desfeito:
/// - o braco `Origem::Projeto if c.so_do_operador()` da `carga::carregar` retirado: a url
///   do projeto vence, o `destino` a recusa e o operador recebe ZERO pedidos;
/// - `rota::destino` reduzido a `Ok(())`: o falso «de fora» recebe o pedido com o
///   `Authorization`.
#[test]
fn api_nao_manda_o_token_para_a_url_do_projeto_nem_para_http_de_fora() {
    let d = pasta("a3");
    let ag = d.join("agente");
    let proj = d.join("proj");
    std::fs::create_dir_all(proj.join(".phxclaw")).unwrap();
    let fora = falso();
    let operador = falso();
    let url_fora = format!("http://0.0.0.0:{}", fora.porta);
    std::fs::write(
        proj.join(".phxclaw/config.json"),
        format!(r#"{{"revisao": 1, "api": {{"url_cliente": "{url_fora}"}}}}"#),
    )
    .unwrap();
    std::fs::create_dir_all(&ag).unwrap();
    std::fs::write(
        ag.join("config.json"),
        format!(
            r#"{{"revisao": 1, "api": {{"url_cliente": "http://127.0.0.1:{}"}}}}"#,
            operador.porta
        ),
    )
    .unwrap();
    let o = cmd(&proj)
        .args(["projeto", "confiar", proj.to_str().unwrap()])
        .args(["--pasta", ag.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(o.status.success(), "{}", texto(&o));
    let api = |extra: &[&str]| {
        cmd(&proj)
            .env("PHXCLAW_API_TOKEN", TOKEN_TESTE)
            .env_remove("PHXCLAW_PROJETO")
            .args(["api", "GET", "/health"])
            .args(extra)
            .args(["--pasta", ag.to_str().unwrap()])
            .output()
            .unwrap()
    };
    // 1) Sem --url: a url do projeto confiado e ignorada (com aviso); vai a do operador.
    let o = api(&[]);
    assert!(o.status.success(), "{}", texto(&o));
    assert!(
        texto(&o).contains("api.url_cliente") && texto(&o).contains("operador"),
        "sem o aviso da chave ignorada: {}",
        texto(&o)
    );
    assert_eq!(operador.contas(), (1, 1), "o operador nao recebeu o pedido");
    assert_eq!(fora.contas(), (0, 0), "o pedido foi para a url do projeto");
    // 2) --url http de fora: recusado sem conectar, com ou sem o interruptor.
    for extra in [
        vec!["--url", &url_fora],
        vec!["--url", &url_fora, "--confio-nesta-url"],
    ] {
        let o = api(&extra);
        assert!(!o.status.success(), "{extra:?}: {}", texto(&o));
        assert!(texto(&o).contains("em claro"), "{}", texto(&o));
        assert_eq!(fora.contas(), (0, 0), "{extra:?}: conectou");
    }
    // 3) --url https de fora sem o interruptor: recusado sem conectar, dizendo o interruptor.
    let https_fora = format!("https://0.0.0.0:{}", fora.porta);
    let o = api(&["--url", &https_fora]);
    assert!(!o.status.success(), "{}", texto(&o));
    assert!(texto(&o).contains("--confio-nesta-url"), "{}", texto(&o));
    assert_eq!(fora.contas(), (0, 0), "https sem o interruptor conectou");
    let _ = std::fs::remove_dir_all(&d);
}

/// M4 (09/10/2026): o `servir` so arma o `gatilhos.json` de projeto CONFIADO. Num clone nao
/// confiado o webhook nao existe (404) e o aviso diz o comando para confiar; confiado, o
/// mesmo arquivo arma o gatilho.
///
/// RED medido: `armar_gatilhos` lendo de novo `montagem::pasta_do_projeto()` sem julgar a
/// confianca (defeito reposto) -- o webhook do clone responde 202.
#[test]
fn servir_so_arma_gatilho_de_projeto_confiado() {
    let d = pasta("m4");
    let ag = d.join("agente");
    let proj = d.join("proj");
    std::fs::create_dir_all(proj.join(".phxclaw")).unwrap();
    std::fs::write(
        proj.join(".phxclaw/gatilhos.json"),
        r#"{"webhooks": [{"nome": "w", "objetivo": "diga oi: {corpo}"}]}"#,
    )
    .unwrap();
    let subir_e_disparar = || {
        let porta = std::net::TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        let mut srv = Filho(
            cmd(&proj)
                .env_remove("PHXCLAW_PROJETO")
                .env("PHXCLAW_API_TOKEN", TOKEN_TESTE)
                .args(["servir", "--porta", &porta.to_string()])
                .args(["--pasta", ag.to_str().unwrap()])
                .stdout(Stdio::null())
                .stderr(Stdio::piped())
                .spawn()
                .unwrap(),
        );
        let url = format!("http://127.0.0.1:{porta}");
        let api = |args: &[&str]| {
            cmd(&proj)
                .env("PHXCLAW_API_TOKEN", TOKEN_TESTE)
                .arg("api")
                .args(args)
                .args(["--url", &url, "--pasta", ag.to_str().unwrap()])
                .output()
                .unwrap()
        };
        let inicio = Instant::now();
        while !api(&["GET", "/health"]).status.success() {
            assert!(
                inicio.elapsed() < Duration::from_secs(60),
                "servidor nao subiu"
            );
            std::thread::sleep(Duration::from_millis(200));
        }
        let o = api(&["POST", "/v1/triggers/w", "--corpo", r#"{"a":1}"#]);
        let _ = srv.0.kill();
        let mut e = String::new();
        srv.0.stderr.take().unwrap().read_to_string(&mut e).unwrap();
        (texto(&o), e)
    };
    let (r, log) = subir_e_disparar();
    assert!(
        r.contains("HTTP 404"),
        "gatilho de clone nao confiado armou: {r}"
    );
    assert!(
        log.contains("gatilhos.json ignorado: projeto nao confiado"),
        "sem o aviso: {log}"
    );
    let o = cmd(&proj)
        .args(["projeto", "confiar", proj.to_str().unwrap()])
        .args(["--pasta", ag.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(o.status.success(), "{}", texto(&o));
    let (r, _) = subir_e_disparar();
    assert!(r.contains("HTTP 202"), "confiado, o gatilho nao armou: {r}");
    let _ = std::fs::remove_dir_all(&d);
}
