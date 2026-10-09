//! Pedido 652 -- na 0.19 o servidor ACEITA o Noise e o TLS na porta de dados,
//! e REGISTRA no log, por par, quem ainda chegou pelo Noise (decisao do dono,
//! 01/10/2026, `plano-tls13-572` T6b-2).
//!
//! # Por que o `phxsqld` de verdade
//!
//! O que se prova e uma linha do ERRO PADRAO do processo -- o log que o
//! operador le. Num teste dentro do processo o `eprintln!` se mistura ao da
//! bateria e nao se le de volta; o binario, com o `stderr` num arquivo, diz
//! exatamente o que o operador veria.
//!
//! # O que se prova, nos dois sentidos
//!
//! 1. a conexao por TLS NAO gera a linha;
//! 2. a conexao por Noise e ATENDIDA (o comportamento velho) e gera a linha,
//!    com o par, o iniciador e a frase do dono;
//! 3. o mesmo par reconectando nao gera a segunda (silencio por par).
//!
//! O vermelho: sem a chamada de `avisar_que_o_noise_acaba` no
//! `responder_aperto`, o passo 2 cai; sem o silencio por par, o 3.

mod comum;
use comum::{DirTemp, Filho};

use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Duration;

use phxsql_core::base64;
use phxsql_core::fio::{Canal, Iniciador, Recebido};
use phxsql_core::json::Json;

const TOKEN: &str = "o token do aviso do noise";
const FRASE: &str = "o Noise será recusado na 0.21; use TLS";

fn ping() -> String {
    format!(r#"{{"token":"{TOKEN}","op":"ping"}}"#)
}

/// Um ping pelo Noise: o aperto em claro e o pedido dentro do tunel.
fn ping_por_noise(porta: u16) -> String {
    let alvo: SocketAddr = format!("127.0.0.1:{porta}").parse().unwrap();
    let fluxo = TcpStream::connect_timeout(&alvo, Duration::from_secs(3)).unwrap();
    fluxo
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    let mut escrita = fluxo.try_clone().unwrap();
    let mut leitor = BufReader::new(fluxo);
    let (iniciador, m1) = Iniciador::comecar(None);
    writeln!(
        escrita,
        r#"{{"op":"cifrar","e":"{}"}}"#,
        base64::codificar(&m1)
    )
    .unwrap();
    let mut resposta = String::new();
    leitor.read_line(&mut resposta).unwrap();
    let j = Json::analisar(&resposta).unwrap();
    assert!(
        j.booleano_ou("ok", false),
        "o aperto Noise foi recusado: {resposta}"
    );
    let m2 = base64::decodificar(j.campo("resultado").unwrap().texto_ou("m2", "")).unwrap();
    let (t, _) = iniciador.terminar(&m2).unwrap();
    let mut canal = Canal::Cifrado(Box::new(t));
    canal.escrever(&mut escrita, &ping()).unwrap();
    match canal.ler(&mut leitor) {
        Ok(Recebido::Linha(l)) => l,
        Ok(Recebido::Fim) => panic!("o tunel Noise encerrou sem responder"),
        Err(e) => panic!("o tunel Noise nao respondeu: {e}"),
    }
}

/// Um ping pelo TLS, pelo `openssl s_client` conferindo o certificado.
fn ping_por_tls(porta: u16, ca: &Path) -> String {
    let mut filho = Command::new("openssl")
        .args(["s_client", "-connect", &format!("127.0.0.1:{porta}")])
        .args(["-tls1_3", "-quiet", "-verify_return_error", "-CAfile"])
        .arg(ca)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut entrada = filho.stdin.take().unwrap();
    entrada
        .write_all(format!("{}\n", ping()).as_bytes())
        .unwrap();
    let saida = filho.stdout.take().unwrap();
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        for l in BufReader::new(saida).lines().map_while(Result::ok) {
            if l.starts_with('{') {
                let _ = tx.send(l);
                return;
            }
        }
    });
    let resposta = rx.recv_timeout(Duration::from_secs(20)).unwrap_or_default();
    let _ = filho.kill();
    let _ = filho.wait();
    drop(entrada);
    resposta
}

fn linhas_do_noise(erro_padrao: &Path) -> Vec<String> {
    std::fs::read_to_string(erro_padrao)
        .unwrap_or_default()
        .lines()
        .filter(|l| l.contains(FRASE))
        .map(str::to_string)
        .collect()
}

#[test]
fn quem_chega_por_noise_e_registrado_uma_vez_por_par_e_o_tls_nao() {
    let d = DirTemp::novo("aviso-do-noise");
    let config = d.join("config.json");
    let bar = |p: std::path::PathBuf| p.display().to_string().replace('\\', "/");
    std::fs::write(
        &config,
        format!(
            r#"{{
              "bind": "127.0.0.1:0", "base": "{}", "token": "{TOKEN}",
              "log_acessos": "{}", "seguranca": {{ "blacklist": "{}" }},
              "dblink": "{}", "jobs": "{}", "web": {{ "ligado": false }},
              "tls": true
            }}"#,
            bar(d.join("base")),
            bar(d.join("acessos.log")),
            bar(d.join("blacklist.json")),
            bar(d.join("dblink.json")),
            bar(d.join("jobs.json")),
        ),
    )
    .unwrap();
    let erro_padrao = d.join("stderr.txt");
    let mut filho = Filho(
        Command::new(env!("CARGO_BIN_EXE_phxsqld"))
            .arg("--config")
            .arg(&config)
            .current_dir(&*d)
            .stdout(Stdio::null())
            .stderr(Stdio::from(std::fs::File::create(&erro_padrao).unwrap()))
            .spawn()
            .expect("nao consegui iniciar o phxsqld"),
    );
    let porta = comum::porta_do_phxsqld(&mut filho, &erro_padrao).unwrap_or_else(|e| panic!("{e}"));

    // 1. TLS: atendido, e sem linha.
    let ca = d.join("tls-dados-certificado.pem");
    assert!(ca.exists(), "o autoassinado da porta de dados nao nasceu");
    let r = ping_por_tls(porta, &ca);
    assert!(
        r.contains(r#""ok":true"#),
        "o ping por TLS nao foi atendido: {r:?}"
    );
    assert!(
        linhas_do_noise(&erro_padrao).is_empty(),
        "a conexao por TLS foi registrada como Noise"
    );

    // 2. Noise: ATENDIDO (a 0.19 nao recusa) e registrado. A linha sai
    //    depois da resposta do aperto, entao espera-se por ela.
    let r = ping_por_noise(porta);
    assert!(
        r.contains(r#""ok":true"#),
        "o ping por Noise nao foi atendido: {r}"
    );
    let mut linhas = Vec::new();
    for _ in 0..200 {
        linhas = linhas_do_noise(&erro_padrao);
        if !linhas.is_empty() {
            break;
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    assert_eq!(
        linhas.len(),
        1,
        "a conexao por Noise nao gerou a linha: {linhas:?}"
    );
    let l = &linhas[0];
    assert!(
        l.contains("127.0.0.1") && l.contains("iniciador: cliente"),
        "{l}"
    );
    // Sem caminho e sem segredo: o par, o iniciador e a frase, e mais nada.
    assert!(!l.contains('/') && !l.contains(TOKEN), "{l}");

    // 3. O mesmo par reconectando: atendido, e calado.
    for _ in 0..3 {
        let r = ping_por_noise(porta);
        assert!(r.contains(r#""ok":true"#), "{r}");
    }
    std::thread::sleep(Duration::from_millis(200));
    assert_eq!(
        linhas_do_noise(&erro_padrao).len(),
        1,
        "o mesmo par reconectando virou uma linha por conexao"
    );
}
