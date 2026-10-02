//! Ida e volta pelo stdio do binario de verdade: initialize le o `.phxclaw/snippets.json`
//! da raiz, didOpen + completion devolvem o snippet com `insertTextFormat: 2`, shutdown e
//! exit encerram o processo com codigo 0.

use serde_json::{Value, json};
use std::io::BufReader;
use std::process::{Command, Stdio};

#[test]
fn initialize_e_completion_devolvem_o_snippet_do_usuario() {
    let raiz = std::env::temp_dir().join(format!("phx-snip-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&raiz);
    std::fs::create_dir_all(raiz.join(".phxclaw")).unwrap();
    std::fs::write(
        raiz.join(".phxclaw/snippets.json"),
        r#"[{"prefixo":"fnmain","corpo":["fn main() {","    $0","}"],"descricao":"main","linguagens":["rust"]}]"#,
    )
    .unwrap();
    let mut p = Command::new(env!("CARGO_BIN_EXE_phxclaw-snippet-ls"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .unwrap();
    let mut entrada = p.stdin.take().unwrap();
    let mut saida = BufReader::new(p.stdout.take().unwrap());
    let mut mandar = |v: Value| {
        phxclaw_snippet_ls::escrever_mensagem(&mut entrada, &v).unwrap();
    };
    mandar(
        json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{
        "rootUri": format!("file://{}", raiz.display()), "capabilities": {}}}),
    );
    let r = phxclaw_snippet_ls::ler_mensagem(&mut saida)
        .unwrap()
        .unwrap();
    assert_eq!(r["id"], 1, "{r}");
    assert!(
        r["result"]["capabilities"]["completionProvider"].is_object(),
        "{r}"
    );
    mandar(json!({"jsonrpc":"2.0","method":"initialized","params":{}}));
    mandar(
        json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{
        "uri":"file:///x/main.rs","languageId":"rust","version":1,"text":"fnm"}}}),
    );
    mandar(
        json!({"jsonrpc":"2.0","id":2,"method":"textDocument/completion","params":{
        "textDocument":{"uri":"file:///x/main.rs"},"position":{"line":0,"character":3}}}),
    );
    let r = phxclaw_snippet_ls::ler_mensagem(&mut saida)
        .unwrap()
        .unwrap();
    assert_eq!(r["id"], 2, "{r}");
    let item = &r["result"]["items"][0];
    assert_eq!(item["label"], "fnmain", "{r}");
    assert_eq!(item["insertTextFormat"], 2, "{r}");
    assert_eq!(item["textEdit"]["newText"], "fn main() {\n    $0\n}", "{r}");
    mandar(json!({"jsonrpc":"2.0","id":3,"method":"shutdown"}));
    let r = phxclaw_snippet_ls::ler_mensagem(&mut saida)
        .unwrap()
        .unwrap();
    assert_eq!(r["id"], 3);
    mandar(json!({"jsonrpc":"2.0","method":"exit"}));
    drop(entrada);
    assert!(p.wait().unwrap().success());
    let _ = std::fs::remove_dir_all(&raiz);
}
