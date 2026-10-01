//! `phxclaw mcp-serve` pelo processo de verdade: o cliente fala MCP pelo stdin/stdout do
//! binario, como o Claude Desktop ou outro agente falaria.

use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

struct Sessao {
    filho: Child,
    entrada: ChildStdin,
    saida: BufReader<ChildStdout>,
    n: i64,
}

impl Sessao {
    fn subir(capacidades: &str) -> (Self, std::path::PathBuf) {
        let raiz = std::env::temp_dir().join(format!("phx-mcp-serve-{}", std::process::id()));
        let raiz = raiz.join(capacidades.replace(',', "_"));
        let trabalho = raiz.join("trabalho");
        std::fs::create_dir_all(&trabalho).unwrap();
        std::fs::write(
            trabalho.join("nota.txt"),
            "conteudo da nota\nsegunda linha\n",
        )
        .unwrap();
        let mut filho = Command::new(env!("CARGO_BIN_EXE_phxclaw"))
            .args(["mcp-serve", "--trabalho"])
            .arg(&trabalho)
            .arg("--pasta")
            .arg(raiz.join("home"))
            .env("PHXCLAW_CAPACIDADES", capacidades)
            .env_remove("PHXCLAW_MCP_CONFIG")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap();
        let entrada = filho.stdin.take().unwrap();
        let saida = BufReader::new(filho.stdout.take().unwrap());
        (
            Self {
                filho,
                entrada,
                saida,
                n: 0,
            },
            raiz,
        )
    }

    fn pede(&mut self, metodo: &str, params: Value) -> Value {
        self.n += 1;
        let m = json!({"jsonrpc":"2.0","id":self.n,"method":metodo,"params":params});
        writeln!(self.entrada, "{m}").unwrap();
        self.entrada.flush().unwrap();
        let mut linha = String::new();
        self.saida.read_line(&mut linha).unwrap();
        let v: Value = serde_json::from_str(&linha).unwrap_or_else(|e| panic!("{e}: {linha}"));
        assert_eq!(v["id"], json!(self.n), "{v}");
        v
    }

    fn avisa(&mut self, metodo: &str) {
        writeln!(self.entrada, "{}", json!({"jsonrpc":"2.0","method":metodo})).unwrap();
    }

    fn nomes(&mut self) -> Vec<String> {
        let r = self.pede("tools/list", json!({}));
        r["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["name"].as_str().unwrap().to_string())
            .collect()
    }

    fn fechar(mut self) {
        drop(self.entrada);
        let st = self.filho.wait().unwrap();
        assert!(st.success(), "mcp-serve saiu com {st}");
    }
}

/// O principal: initialize + tools/list + tools/call de `read_file` numa pasta, e a
/// ferramenta sem capacidade concedida fora da lista e negada pelo mesmo portao do motor.
#[test]
fn mcp_serve_expoe_so_o_que_a_politica_concede() {
    let (mut s, raiz) = Sessao::subir("fs.read");
    let r = s.pede(
        "initialize",
        json!({"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"t","version":"1"}}),
    );
    assert_eq!(r["result"]["protocolVersion"], "2025-06-18", "{r}");
    assert!(r["result"]["capabilities"]["tools"].is_object(), "{r}");
    // Notificacao nao tem resposta: se tivesse, a proxima leitura pegaria a errada.
    s.avisa("notifications/initialized");

    let nomes = s.nomes();
    assert!(nomes.contains(&"read_file".to_string()), "{nomes:?}");
    assert!(!nomes.contains(&"write_file".to_string()), "{nomes:?}");
    assert!(!nomes.contains(&"shell".to_string()), "{nomes:?}");

    let r = s.pede(
        "tools/call",
        json!({"name":"read_file","arguments":{"path":"nota.txt"}}),
    );
    assert_eq!(r["result"]["isError"], false, "{r}");
    let texto = r["result"]["content"][0]["text"].as_str().unwrap();
    assert!(texto.contains("conteudo da nota"), "{texto}");

    // Pedida pelo nome, a que nao esta na lista volta negada e nao grava nada.
    let r = s.pede(
        "tools/call",
        json!({"name":"write_file","arguments":{"path":"x.txt","content":"nao"}}),
    );
    assert_eq!(r["result"]["isError"], true, "{r}");
    let texto = r["result"]["content"][0]["text"].as_str().unwrap();
    assert!(
        texto.contains("NEGADO") && texto.contains("fs.write"),
        "{texto}"
    );
    assert!(!raiz.join("trabalho/x.txt").exists());

    let r = s.pede("metodo/inexistente", json!({}));
    assert_eq!(r["error"]["code"], -32601, "{r}");
    s.fechar();
}

/// O mesmo binario com `fs.write` concedida passa a listar e a gravar: a lista muda com a
/// politica, nao com o codigo.
#[test]
fn mcp_serve_segue_o_phxclaw_capacidades() {
    let (mut s, raiz) = Sessao::subir("fs.read,fs.write");
    s.pede("initialize", json!({"protocolVersion":"2025-11-25"}));
    let nomes = s.nomes();
    assert!(nomes.contains(&"write_file".to_string()), "{nomes:?}");
    let r = s.pede(
        "tools/call",
        json!({"name":"write_file","arguments":{"path":"x.txt","content":"sim"}}),
    );
    assert_eq!(r["result"]["isError"], false, "{r}");
    assert_eq!(
        std::fs::read_to_string(raiz.join("trabalho/x.txt")).unwrap(),
        "sim"
    );
    s.fechar();
}
