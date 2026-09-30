//! E-mail pelo fio SMTP de verdade, contra um servidor SMTP minimo local.

use phxclaw_agent::email::{EmailTool, SmtpConfig, SmtpSecurity};
use phxclaw_agent_core::{Tool, ToolContext, ToolError};
use serde_json::json;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::time::Duration;

type Caixa = Arc<Mutex<Vec<String>>>;

/// Servidor SMTP que aceita tudo e guarda (envelope, dados) de cada mensagem.
fn smtp() -> (u16, Caixa, Arc<Mutex<usize>>) {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let porta = l.local_addr().unwrap().port();
    let msgs = Arc::new(Mutex::new(vec![]));
    let conexoes = Arc::new(Mutex::new(0));
    let (m2, c2) = (msgs.clone(), conexoes.clone());
    std::thread::spawn(move || {
        for s in l.incoming() {
            let Ok(s) = s else { break };
            *c2.lock().unwrap() += 1;
            let mut w = s.try_clone().unwrap();
            let mut r = BufReader::new(s);
            let _ = w.write_all(b"220 teste ESMTP\r\n");
            let (mut envelope, mut dados, mut em_dados) = (String::new(), String::new(), false);
            let mut linha = String::new();
            while r.read_line(&mut linha).unwrap_or(0) > 0 {
                if em_dados {
                    if linha == ".\r\n" {
                        em_dados = false;
                        m2.lock().unwrap().push(format!("{envelope}\n{dados}"));
                        let _ = w.write_all(b"250 2.0.0 aceito\r\n");
                    } else {
                        dados.push_str(&linha);
                    }
                } else {
                    let cmd = linha.to_ascii_uppercase();
                    let resp: &[u8] = if cmd.starts_with("EHLO") {
                        b"250-teste\r\n250 8BITMIME\r\n"
                    } else if cmd.starts_with("DATA") {
                        em_dados = true;
                        b"354 manda\r\n"
                    } else if cmd.starts_with("QUIT") {
                        let _ = w.write_all(b"221 tchau\r\n");
                        break;
                    } else {
                        envelope.push_str(&linha);
                        b"250 ok\r\n"
                    };
                    let _ = w.write_all(resp);
                }
                linha.clear();
            }
        }
    });
    (porta, msgs, conexoes)
}

fn cfg(porta: u16, host: &str, security: SmtpSecurity) -> SmtpConfig {
    SmtpConfig {
        host: host.into(),
        port: porta,
        security,
        username: None,
        password: None,
        from: "PhxClaw <agente@phxclaw.local>".into(),
        allowed_recipients: vec!["@empresa.com.br".into(), "dono@x.com".into()],
    }
}

fn ctx() -> ToolContext {
    let d = std::env::temp_dir().join(format!("phx-mail-{}", phxclaw_types::new_uuid_v7()));
    std::fs::create_dir_all(&d).unwrap();
    std::fs::write(d.join("relatorio.md"), "# Relatório\nConclusão & dados").unwrap();
    ToolContext {
        task_id: "t".into(),
        workdir: d,
        timeout: Duration::from_secs(10),
    }
}

#[tokio::test]
async fn envia_com_anexo_pelo_fio_smtp() {
    let (porta, msgs, _) = smtp();
    let t = EmailTool {
        config: cfg(porta, "127.0.0.1", SmtpSecurity::Plain),
    };
    let r = t
        .run(json!({"to": ["ana@empresa.com.br"], "subject": "Relatório semanal", "body": "Segue o relatório.", "attachments": ["relatorio.md"]}), &ctx())
        .await
        .unwrap();
    assert!(
        r.content.contains("enviado para ana@empresa.com.br"),
        "{}",
        r.content
    );
    let m = msgs.lock().unwrap()[0].clone();
    assert!(m.contains("RCPT TO:<ana@empresa.com.br>"), "{m}");
    assert!(m.contains("filename=\"relatorio.md\""), "{m}");
    assert!(
        m.contains("Subject: =?utf-8?"),
        "assunto com acento tem de ir codificado: {m}"
    );
}

#[tokio::test]
async fn destinatario_fora_da_lista_e_negado_sem_abrir_conexao() {
    let (porta, _, conexoes) = smtp();
    let t = EmailTool {
        config: cfg(porta, "127.0.0.1", SmtpSecurity::Plain),
    };
    let e = t
        .run(
            json!({"to": ["ana@empresa.com.br", "atacante@fora.net"], "subject": "x", "body": "y"}),
            &ctx(),
        )
        .await
        .unwrap_err();
    assert!(matches!(e, ToolError::Denied(_)), "{e}");
    assert_eq!(*conexoes.lock().unwrap(), 0);
}

#[tokio::test]
async fn anexo_fora_da_pasta_e_smtp_sem_tls_fora_do_loopback_sao_negados() {
    let (porta, _, _) = smtp();
    let t = EmailTool {
        config: cfg(porta, "127.0.0.1", SmtpSecurity::Plain),
    };
    let e = t.run(json!({"to": ["dono@x.com"], "subject": "x", "body": "y", "attachments": ["../../etc/passwd"]}), &ctx()).await.unwrap_err();
    assert!(matches!(e, ToolError::Denied(_)), "{e}");
    let t = EmailTool {
        config: cfg(porta, "smtp.exemplo.com", SmtpSecurity::Plain),
    };
    let e = t
        .run(
            json!({"to": ["dono@x.com"], "subject": "x", "body": "y"}),
            &ctx(),
        )
        .await
        .unwrap_err();
    assert!(matches!(e, ToolError::Denied(_)), "{e}");
    // e a senha nunca aparece no Debug da configuracao
    let mut c = cfg(porta, "h", SmtpSecurity::Tls);
    c.password = Some("s3nh4-secreta".into());
    assert!(!format!("{c:?}").contains("s3nh4"));
}
