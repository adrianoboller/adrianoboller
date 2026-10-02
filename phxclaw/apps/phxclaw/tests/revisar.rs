//! `phxclaw revisar` e `phxclaw forja token` pelo processo de verdade: o diff sai do git
//! real (no bwrap) de um repositorio temporario, a revisao passa pelo motor do
//! `code_review`, e o modelo e um Ollama FALSO local (sem modelo de verdade aqui).

use phxclaw_test_support::pulado;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};

fn tmp(nome: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!(
        "phx-cli-rev-{nome}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// Ollama falso: responde a todo `POST /api/chat` com `conteudo` e guarda os corpos.
fn ollama_falso(conteudo: String) -> (String, Arc<Mutex<Vec<String>>>) {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", l.local_addr().unwrap());
    let vistos: Arc<Mutex<Vec<String>>> = Arc::default();
    let v2 = vistos.clone();
    std::thread::spawn(move || {
        for s in l.incoming().flatten() {
            let mut r = BufReader::new(s.try_clone().unwrap());
            let mut tamanho = 0usize;
            loop {
                let mut linha = String::new();
                if r.read_line(&mut linha).unwrap_or(0) == 0 || linha == "\r\n" {
                    break;
                }
                if let Some(n) = linha.to_lowercase().strip_prefix("content-length:") {
                    tamanho = n.trim().parse().unwrap_or(0);
                }
            }
            let mut corpo = vec![0; tamanho];
            let _ = r.read_exact(&mut corpo);
            v2.lock()
                .unwrap()
                .push(String::from_utf8_lossy(&corpo).into());
            let resp = serde_json::json!({
                "model": "falso",
                "message": {"role": "assistant", "content": conteudo},
                "prompt_eval_count": 10, "eval_count": 5, "done": true
            })
            .to_string();
            let mut s = s;
            let _ = write!(
                s,
                "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{resp}",
                resp.len()
            );
        }
    });
    (base, vistos)
}

fn git(dir: &Path, args: &[&str]) {
    let o = Command::new("git")
        .args([
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@t",
            "-c",
            "init.defaultBranch=main",
        ])
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap();
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
}

#[test]
fn revisar_le_o_diff_do_git_e_falha_na_severidade_pedida() {
    if phxclaw_bwrap().is_none() {
        pulado::pular("bwrap", "sem bwrap a prova da revisao nao roda");
        return;
    }
    let repo = tmp("repo");
    git(&repo, &["init", "-q"]);
    std::fs::write(repo.join("m.py"), "a = 1\n").unwrap();
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-qm", "base"]);
    std::fs::write(repo.join("m.py"), "a = 1\nsenha = 'x'\n").unwrap();
    let (base, vistos) = ollama_falso(
        r#"{"resumo":"segredo","achados":[{"arquivo":"m.py","linha":2,"severidade":"critica","achado":"senha no codigo"},{"arquivo":"m.py","linha":40,"severidade":"baixa","achado":"inventado"}]}"#
            .into(),
    );
    let o = Command::new(env!("CARGO_BIN_EXE_phxclaw"))
        .args(["revisar", "--repo"])
        .arg(&repo)
        .args(["--modelo", "ollama:falso", "--falhar-em", "alta"])
        .env("OLLAMA_HOST", &base)
        .output()
        .unwrap();
    let saida = String::from_utf8_lossy(&o.stdout);
    assert_eq!(
        o.status.code(),
        Some(1),
        "{saida}\n{}",
        String::from_utf8_lossy(&o.stderr)
    );
    let v: serde_json::Value = serde_json::from_str(&saida).unwrap();
    assert_eq!(v["achados"].as_array().unwrap().len(), 1, "{v}");
    assert_eq!(v["achados"][0]["linha"], 2);
    assert_eq!(v["descartados"][0]["motivo"], "linha fora do diff");
    // O modelo recebeu o diff do git, numerado.
    let pedido = vistos.lock().unwrap()[0].clone();
    assert!(
        pedido.contains("senha = 'x'") && pedido.contains("     2 +"),
        "{pedido}"
    );

    // Sem achado na severidade pedida: sai 0.
    let (base, _) = ollama_falso(r#"{"resumo":"ok","achados":[]}"#.into());
    let o = Command::new(env!("CARGO_BIN_EXE_phxclaw"))
        .args(["revisar", "--repo"])
        .arg(&repo)
        .args(["--modelo", "ollama:falso", "--falhar-em", "alta"])
        .env("OLLAMA_HOST", &base)
        .output()
        .unwrap();
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
}

#[test]
fn forja_token_guarda_no_broker_sem_texto_claro() {
    let pasta = tmp("forja");
    let o = Command::new(env!("CARGO_BIN_EXE_phxclaw"))
        .args(["forja", "token", "github", "--pasta"])
        .arg(&pasta)
        .env("PHXCLAW_GITHUB_TOKEN", "ghp_CLI_SEGREDO")
        .output()
        .unwrap();
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    let tudo = String::from_utf8_lossy(&o.stdout).to_string() + &String::from_utf8_lossy(&o.stderr);
    assert!(!tudo.contains("CLI_SEGREDO"), "{tudo}");
    let mut pilha = vec![pasta.join("forja")];
    let mut arquivos = 0;
    while let Some(d) = pilha.pop() {
        for e in std::fs::read_dir(&d).unwrap().flatten() {
            if e.path().is_dir() {
                pilha.push(e.path());
            } else {
                arquivos += 1;
                let b = std::fs::read(e.path()).unwrap();
                assert!(
                    !String::from_utf8_lossy(&b).contains("CLI_SEGREDO"),
                    "{}",
                    e.path().display()
                );
            }
        }
    }
    assert!(arquivos > 0);
    // Sem a variavel: recusa dizendo qual falta.
    let o = Command::new(env!("CARGO_BIN_EXE_phxclaw"))
        .args(["forja", "token", "gitlab", "--pasta"])
        .arg(&pasta)
        .env_remove("PHXCLAW_GITLAB_TOKEN")
        .output()
        .unwrap();
    assert!(!o.status.success());
    assert!(String::from_utf8_lossy(&o.stderr).contains("PHXCLAW_GITLAB_TOKEN"));
}

fn phxclaw_bwrap() -> Option<PathBuf> {
    ["/usr/bin/bwrap", "/bin/bwrap", "/usr/local/bin/bwrap"]
        .iter()
        .map(PathBuf::from)
        .find(|p| p.exists())
}

// ------------------------------------------------------------------ GitHub Action

/// GitHub falso: o diff do PR 7 (so com o Bearer certo) e o POST do comentario, guardado.
fn github_falso(token: &'static str) -> (String, Arc<Mutex<Vec<String>>>) {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", l.local_addr().unwrap());
    let comentarios: Arc<Mutex<Vec<String>>> = Arc::default();
    let c2 = comentarios.clone();
    std::thread::spawn(move || {
        for s in l.incoming().flatten() {
            let mut r = BufReader::new(s.try_clone().unwrap());
            let mut primeira = String::new();
            r.read_line(&mut primeira).unwrap_or(0);
            let (mut tamanho, mut auth) = (0usize, String::new());
            loop {
                let mut linha = String::new();
                if r.read_line(&mut linha).unwrap_or(0) == 0 || linha == "\r\n" {
                    break;
                }
                let baixa = linha.to_lowercase();
                if let Some(n) = baixa.strip_prefix("content-length:") {
                    tamanho = n.trim().parse().unwrap_or(0);
                }
                if baixa.starts_with("authorization:") {
                    auth = linha["authorization:".len()..].trim().to_string();
                }
            }
            let mut corpo = vec![0; tamanho];
            let _ = r.read_exact(&mut corpo);
            let (status, resp) = if auth != format!("Bearer {token}") {
                (
                    "401 Unauthorized",
                    r#"{"message":"Bad credentials"}"#.to_string(),
                )
            } else if primeira.starts_with("GET /repos/dono/proj/pulls/7 ") {
                (
                    "200 OK",
                    "diff --git a/m.py b/m.py\n--- a/m.py\n+++ b/m.py\n@@ -1 +1,2 @@\n a = 1\n+senha = 'x'\n"
                        .to_string(),
                )
            } else if primeira.starts_with("POST /repos/dono/proj/issues/7/comments ") {
                c2.lock()
                    .unwrap()
                    .push(String::from_utf8_lossy(&corpo).into());
                (
                    "201 Created",
                    r#"{"id":1,"html_url":"https://x/1"}"#.to_string(),
                )
            } else {
                ("404 Not Found", r#"{"message":"Not Found"}"#.to_string())
            };
            let mut s = s;
            let _ = write!(
                s,
                "HTTP/1.1 {status}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{resp}",
                resp.len()
            );
        }
    });
    (base, comentarios)
}

/// Os blocos `run: |` da acao composta, na ordem dos passos.
fn passos_da_acao(yml: &str) -> Vec<String> {
    let mut passos = Vec::new();
    let mut atual: Option<(usize, String)> = None;
    for linha in yml.lines() {
        let recuo = linha.len() - linha.trim_start().len();
        if let Some((r, s)) = &mut atual {
            if linha.trim().is_empty() || recuo > *r {
                s.push_str(linha.get(*r + 2..).unwrap_or(""));
                s.push('\n');
                continue;
            }
            passos.push(std::mem::take(s));
            atual = None;
        }
        if linha.trim() == "run: |" {
            atual = Some((recuo, String::new()));
        }
    }
    if let Some((_, s)) = atual {
        passos.push(s);
    }
    passos
}

/// A acao composta de verdade: os tres `run` do action.yml rodam no bash com o ambiente que
/// o runner daria (GITHUB_EVENT_PATH, GITHUB_ENV, RUNNER_TEMP), contra um GitHub e um
/// Ollama falsos. Prova o caminho evento -> `phxclaw revisar --evento` -> comentario.
#[test]
fn acao_do_github_revisa_o_pr_do_evento_e_comenta() {
    const TOKEN: &str = "ghs_TOKEN_DA_ACAO_QUE_NAO_VAZA";
    let yml = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.github/actions/phxclaw/action.yml"),
    )
    .unwrap();
    assert!(yml.contains("using: composite"));
    let passos = passos_da_acao(&yml);
    assert_eq!(passos.len(), 3, "{passos:?}");
    for chave in [
        "PHXCLAW_BIN_ENTRADA:",
        "PHXCLAW_GITHUB_TOKEN:",
        "PHXCLAW_GITHUB_API:",
        "PHXCLAW_MODELO:",
    ] {
        assert!(yml.contains(chave), "o action.yml perdeu {chave}");
    }
    assert!(!yml.contains("run: ${{"), "entrada interpolada no script");

    let pasta = tmp("acao");
    let (gh, comentarios) = github_falso(TOKEN);
    let (ollama, _) = ollama_falso(
        r#"{"resumo":"segredo no codigo","achados":[{"arquivo":"m.py","linha":2,"severidade":"alta","achado":"senha | literal","sugestao":"use o broker"}]}"#
            .into(),
    );
    let rodar = |evento: &str| {
        let ev = pasta.join("evento.json");
        std::fs::write(&ev, evento).unwrap();
        let genv = pasta.join("github_env");
        std::fs::write(&genv, "").unwrap();
        let mut saida = String::new();
        let mut bin = String::new();
        for (i, p) in passos.iter().enumerate() {
            let mut c = Command::new("bash");
            c.arg("-c")
                .arg(p)
                .env("GITHUB_ENV", &genv)
                .env("GITHUB_EVENT_PATH", &ev)
                .env("GITHUB_EVENT_NAME", "pull_request")
                .env("RUNNER_TEMP", &pasta)
                .env("PHXCLAW_BIN_ENTRADA", env!("CARGO_BIN_EXE_phxclaw"))
                .env("PHXCLAW_GITHUB_TOKEN", TOKEN)
                .env("PHXCLAW_GITHUB_API", &gh)
                .env("PHXCLAW_MODELO", "ollama:falso")
                .env("PHXCLAW_FOCO", "")
                .env("PHXCLAW_FALHAR_EM", "")
                .env("OLLAMA_HOST", &ollama);
            if !bin.is_empty() {
                c.env("PHXCLAW_BIN", &bin);
            }
            let o = c.output().unwrap();
            let tudo = String::from_utf8_lossy(&o.stdout).to_string()
                + &String::from_utf8_lossy(&o.stderr);
            assert!(o.status.success(), "passo {i}: {tudo}");
            assert!(!tudo.contains(TOKEN), "passo {i} vazou o token: {tudo}");
            saida.push_str(&tudo);
            if i == 0 {
                // O primeiro passo passa o binario aos outros pelo GITHUB_ENV, como no runner.
                bin = std::fs::read_to_string(&genv)
                    .unwrap()
                    .lines()
                    .find_map(|l| l.strip_prefix("PHXCLAW_BIN=").map(str::to_string))
                    .expect("o passo 1 nao exportou PHXCLAW_BIN");
            }
        }
        saida
    };

    let saida = rodar(
        r#"{"action":"opened","number":7,"pull_request":{"number":7,"draft":false},"repository":{"full_name":"dono/proj"}}"#,
    );
    assert!(saida.contains("senha | literal"), "{saida}");
    let c = comentarios.lock().unwrap().clone();
    assert_eq!(c.len(), 1, "{saida}");
    let corpo: serde_json::Value = serde_json::from_str(&c[0]).unwrap();
    let corpo = corpo["body"].as_str().unwrap();
    assert!(corpo.starts_with("### Revisao do PhxClaw"), "{corpo}");
    assert!(
        corpo.contains("| alta | `m.py:2` | senha \\| literal | use o broker |"),
        "{corpo}"
    );

    // PR fechado: nada a revisar, nada comentado, passo verde.
    let saida = rodar(
        r#"{"action":"closed","number":7,"pull_request":{"number":7},"repository":{"full_name":"dono/proj"}}"#,
    );
    assert!(saida.contains("\"revisado\": false"), "{saida}");
    assert_eq!(comentarios.lock().unwrap().len(), 1);
    // O token so existe cifrado no broker da pasta do runner.
    let mut pilha = vec![pasta.clone()];
    while let Some(d) = pilha.pop() {
        for e in std::fs::read_dir(&d).unwrap().flatten() {
            if e.path().is_dir() {
                pilha.push(e.path());
            } else {
                let b = std::fs::read(e.path()).unwrap();
                assert!(
                    !String::from_utf8_lossy(&b).contains(TOKEN),
                    "{}",
                    e.path().display()
                );
            }
        }
    }
}
