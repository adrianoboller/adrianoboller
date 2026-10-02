//! `phxclaw medir` pelo binario (SP000030): soma por tarefa de uma gravacao v2 com um
//! subagente, e a v1 sai «não medido» em vez de estimada.

use std::path::PathBuf;
use std::process::Command;

fn tmp() -> PathBuf {
    let n = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let d = std::env::temp_dir().join(format!("phx-medir-{}-{n}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn medir(arq: &std::path::Path, extra: &[&str]) -> String {
    let out = Command::new(env!("CARGO_BIN_EXE_phxclaw"))
        .arg("medir")
        .arg(arq)
        .args(extra)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).into_owned()
}

/// Reposto o defeito (somar os tokens que existem como se fossem o total), a raiz diria
/// «10 entrada» com uma chamada sem contagem; o certo e dizer que ficou sem.
#[test]
fn medir_soma_por_tarefa_e_nao_estima_o_que_nao_foi_medido() {
    let d = tmp();
    let v2 = d.join("v2.jsonl");
    std::fs::write(&v2, concat!(
        r#"{"tipo":"cabecalho","versao":2,"objetivo":"o","modelo":"m","data":"2026-10-02T00:00:00Z","passo":0}"#, "\n",
        r#"{"tipo":"prompt","versao":2,"prompt_sha256":"abcdef0123456789","skills_sha256":{"relatorio":"ff"},"passo":1}"#, "\n",
        r#"{"tipo":"modelo","versao":2,"tarefa":"raiz","duracao_ms":100.0,"tokens_entrada":10,"tokens_saida":5,"resposta":{"content":"","tool_calls":[],"usage":{"input_tokens":10,"output_tokens":5},"model":"m"},"passo":2}"#, "\n",
        r#"{"tipo":"ferramenta","versao":2,"tarefa":"filha","passo_pai":3,"duracao_ms":20.0,"nome":"soma","argumentos":{},"saida":{"content":"3"},"passo":4}"#, "\n",
        r#"{"tipo":"ferramenta","versao":2,"tarefa":"raiz","duracao_ms":50.0,"nome":"pai","argumentos":{},"saida":{"content":"ok"},"passo":3}"#, "\n",
        r#"{"tipo":"modelo","versao":2,"tarefa":"raiz","duracao_ms":200.0,"resposta":{"content":"fim","tool_calls":[],"usage":{"input_tokens":0,"output_tokens":0},"model":"m"},"passo":5}"#, "\n",
    )).unwrap();
    let s = medir(&v2, &[]);
    assert!(
        s.contains("gravacao v2") && s.contains("prompt abcdef012345") && s.contains("1 skill(s)"),
        "{s}"
    );
    assert!(
        s.contains(
            "tarefa raiz\n  modelo      : 2 chamada(s), 300 ms\n  ferramentas : 1 chamada(s), 50 ms"
        ),
        "{s}"
    );
    assert!(
        s.contains("não informados pelo provedor (1 de 2 chamada(s) sem contagem)"),
        "{s}"
    );
    assert!(s.contains("tarefa filha  (subagente dentro do passo 3)\n  modelo      : 0 chamada(s), 0 ms\n  ferramentas : 1 chamada(s), 20 ms"), "{s}");
    let j: serde_json::Value = serde_json::from_str(&medir(&v2, &["--json"])).unwrap();
    assert_eq!(j.as_array().unwrap().len(), 2);
    assert!(j[0]["tokens_entrada"].is_null());
    assert_eq!(j[1]["passo_pai"], 3);

    let v1 = d.join("v1.jsonl");
    std::fs::write(&v1, concat!(
        r#"{"tipo":"cabecalho","versao":1,"objetivo":"o","modelo":"m","data":"d","passo":0}"#, "\n",
        r#"{"tipo":"ferramenta","nome":"ls","argumentos":{},"saida":{"content":"a"},"passo":1}"#, "\n",
    )).unwrap();
    let s = medir(&v1, &[]);
    assert!(
        s.contains("gravacao v1") && s.contains("prompt não gravado"),
        "{s}"
    );
    assert!(
        s.contains("ferramentas : 1 chamada(s), não medido (gravação v1)"),
        "{s}"
    );
    let _ = std::fs::remove_dir_all(&d);
}
