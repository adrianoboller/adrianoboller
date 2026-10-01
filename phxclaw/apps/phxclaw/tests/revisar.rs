//! `phxclaw revisar` e `phxclaw forja token` pelo processo de verdade: o diff sai do git
//! real (no bwrap) de um repositorio temporario, a revisao passa pelo motor do
//! `code_review`, e o modelo e um Ollama FALSO local (sem modelo de verdade aqui).

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
        eprintln!("sem bwrap: prova pulada");
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
