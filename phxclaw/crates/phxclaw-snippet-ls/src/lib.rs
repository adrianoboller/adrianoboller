#![forbid(unsafe_code)]
//! Servidor de linguagem minimo (JSON-RPC sobre stdio) para o Helix do IDE: serve os
//! snippets do usuario de `.phxclaw/snippets.json` como `CompletionItem` com
//! `insertTextFormat: 2` (o Helix expande os tabstops) e a expansao Emmet de html/css.
//!
//! Por que um servidor nosso e nao um `emmet-ls` de fora: o Helix 25.07.1 so expande
//! snippet que vem por LSP, e o IDE nao arrasta node nem npm. O servidor nao le nada
//! alem do `snippets.json` e do texto que o editor manda; nao grava, nao abre rede.

pub mod emmet;
pub mod ia;

use serde_json::{Value, json};
use std::collections::HashMap;
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};

/// Onde os snippets do usuario moram, a partir da raiz do workspace.
pub const ARQUIVO_DE_SNIPPETS: &str = ".phxclaw/snippets.json";

#[derive(Debug, Clone, PartialEq)]
pub struct Snippet {
    pub prefixo: String,
    pub corpo: String,
    pub descricao: String,
    /// Vazia = vale para toda linguagem.
    pub linguagens: Vec<String>,
}

/// Le o arquivo: uma lista de `{prefixo, corpo, descricao, linguagens}` (ou `{"snippets":
/// [...]}`); `corpo` pode ser texto ou lista de linhas, como no VS Code. Arquivo ausente
/// e lista vazia; invalido e erro dito.
pub fn carregar_snippets(raiz: &Path) -> Result<Vec<Snippet>, String> {
    let arq = raiz.join(ARQUIVO_DE_SNIPPETS);
    let texto = match std::fs::read_to_string(&arq) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(format!("{}: {e}", arq.display())),
    };
    let v: Value = serde_json::from_str(&texto).map_err(|e| format!("{}: {e}", arq.display()))?;
    let lista = match &v {
        Value::Array(a) => a.clone(),
        Value::Object(o) => o
            .get("snippets")
            .and_then(Value::as_array)
            .cloned()
            .ok_or_else(|| {
                format!(
                    "{}: esperava lista ou {{\"snippets\": [...]}}",
                    arq.display()
                )
            })?,
        _ => return Err(format!("{}: esperava lista", arq.display())),
    };
    let mut saida = Vec::new();
    for (n, s) in lista.iter().enumerate() {
        let prefixo = s["prefixo"]
            .as_str()
            .filter(|p| !p.is_empty() && !p.contains(char::is_whitespace))
            .ok_or_else(|| {
                format!(
                    "{}: snippet {n}: prefixo ausente ou com espaco",
                    arq.display()
                )
            })?;
        let corpo = match &s["corpo"] {
            Value::String(c) => c.clone(),
            Value::Array(linhas) => linhas
                .iter()
                .map(|l| l.as_str().unwrap_or_default())
                .collect::<Vec<_>>()
                .join("\n"),
            _ => {
                return Err(format!(
                    "{}: snippet {n} ({prefixo}): corpo ausente",
                    arq.display()
                ));
            }
        };
        saida.push(Snippet {
            prefixo: prefixo.to_string(),
            corpo,
            descricao: s["descricao"].as_str().unwrap_or("").to_string(),
            linguagens: s["linguagens"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(Value::as_str)
                        .map(str::to_string)
                        .collect()
                })
                .unwrap_or_default(),
        });
    }
    Ok(saida)
}

struct Documento {
    linguagem: String,
    texto: String,
}

/// O estado do servidor: raiz, snippets carregados no `initialize` e os documentos
/// abertos (sincronizacao completa, a mais simples que o Helix aceita).
#[derive(Default)]
pub struct Servidor {
    pub raiz: Option<PathBuf>,
    pub snippets: Vec<Snippet>,
    docs: HashMap<String, Documento>,
    pub encerrar: bool,
    /// A completacao por IA, quando o ambiente a configura (ver `ia.rs`). `None` e o
    /// comportamento de sempre: so snippets e Emmet, sem pedido nenhum pela rede.
    pub ia: Option<ia::Configuracao>,
}

/// `CompletionItemKind.Snippet`.
const KIND_SNIPPET: u64 = 15;
/// `InsertTextFormat.Snippet`: o editor interpreta `$1`, `${2:x}`, `$0`.
const FORMATO_SNIPPET: u64 = 2;

/// Texto literal dentro de um snippet LSP: `$` e `\` escapados, senao viram tabstop.
pub fn escapar_snippet(s: &str) -> String {
    s.replace('\\', "\\\\").replace('$', "\\$")
}

impl Servidor {
    /// Trata uma mensagem; devolve a resposta (so para pedidos com `id`).
    pub fn tratar(&mut self, msg: &Value) -> Option<Value> {
        let metodo = msg.get("method").and_then(Value::as_str).unwrap_or("");
        let id = msg.get("id").cloned();
        let params = msg.get("params").cloned().unwrap_or(Value::Null);
        let resultado = match metodo {
            "initialize" => Ok(self.initialize(&params)),
            "initialized" | "$/cancelRequest" | "$/setTrace" => return None,
            "textDocument/didOpen" => {
                let d = &params["textDocument"];
                if let Some(uri) = d["uri"].as_str() {
                    self.docs.insert(
                        uri.to_string(),
                        Documento {
                            linguagem: d["languageId"].as_str().unwrap_or("").to_string(),
                            texto: d["text"].as_str().unwrap_or("").to_string(),
                        },
                    );
                }
                return None;
            }
            "textDocument/didChange" => {
                if let (Some(uri), Some(muds)) = (
                    params["textDocument"]["uri"].as_str(),
                    params["contentChanges"].as_array(),
                ) && let Some(d) = self.docs.get_mut(uri)
                    && let Some(t) = muds.last().and_then(|m| m["text"].as_str())
                {
                    d.texto = t.to_string();
                }
                return None;
            }
            "textDocument/didClose" => {
                if let Some(uri) = params["textDocument"]["uri"].as_str() {
                    self.docs.remove(uri);
                }
                return None;
            }
            "textDocument/didSave" => return None,
            "textDocument/completion" => Ok(self.completion(&params)),
            "shutdown" => Ok(Value::Null),
            "exit" => {
                self.encerrar = true;
                return None;
            }
            outro => Err((-32601, format!("metodo nao suportado: {outro}"))),
        };
        let id = id?;
        Some(match resultado {
            Ok(r) => json!({"jsonrpc": "2.0", "id": id, "result": r}),
            Err((c, m)) => json!({"jsonrpc": "2.0", "id": id, "error": {"code": c, "message": m}}),
        })
    }

    fn initialize(&mut self, p: &Value) -> Value {
        let raiz = p["rootUri"]
            .as_str()
            .and_then(caminho_de_uri)
            .or_else(|| p["rootPath"].as_str().map(PathBuf::from))
            .or_else(|| {
                p["workspaceFolders"][0]["uri"]
                    .as_str()
                    .and_then(caminho_de_uri)
            });
        if let Some(r) = &raiz {
            match carregar_snippets(r) {
                Ok(s) => self.snippets = s,
                // stderr e o unico canal fora do protocolo; so o caminho e o motivo, nunca
                // o conteudo do arquivo.
                Err(e) => eprintln!("phxclaw-snippet-ls: {e}"),
            }
        }
        self.raiz = raiz;
        self.ia = ia::do_ambiente();
        json!({
            "capabilities": {
                "textDocumentSync": 1,
                "completionProvider": {
                    "triggerCharacters": [">", "+", "^", "*", ".", "#", "}"],
                    "resolveProvider": false
                }
            },
            "serverInfo": {"name": "phxclaw-snippet-ls", "version": env!("CARGO_PKG_VERSION")}
        })
    }

    fn completion(&self, p: &Value) -> Value {
        let uri = p["textDocument"]["uri"].as_str().unwrap_or("");
        let linha = p["position"]["line"].as_u64().unwrap_or(0) as usize;
        let coluna = p["position"]["character"].as_u64().unwrap_or(0) as usize;
        let Some(doc) = self.docs.get(uri) else {
            return json!([]);
        };
        let texto_linha = doc.texto.lines().nth(linha).unwrap_or("");
        let (inicio, palavra) = palavra_antes(texto_linha, coluna);
        let faixa = |ini: usize| {
            json!({"start": {"line": linha, "character": ini},
                   "end": {"line": linha, "character": coluna}})
        };
        let mut itens = Vec::new();
        for s in &self.snippets {
            let serve = s.linguagens.is_empty() || s.linguagens.contains(&doc.linguagem);
            if serve && !palavra.is_empty() && s.prefixo.starts_with(palavra) {
                itens.push(json!({
                    "label": s.prefixo,
                    "kind": KIND_SNIPPET,
                    "detail": s.descricao,
                    "filterText": s.prefixo,
                    "insertTextFormat": FORMATO_SNIPPET,
                    "textEdit": {"range": faixa(inicio), "newText": s.corpo}
                }));
            }
        }
        let emmet = match doc.linguagem.as_str() {
            "html" => emmet::expandir_html(palavra),
            "css" | "scss" => emmet::expandir_css(palavra),
            _ => None,
        };
        if let Some(e) = emmet {
            itens.push(json!({
                "label": palavra,
                "kind": KIND_SNIPPET,
                "detail": "emmet",
                "filterText": palavra,
                "sortText": "0",
                "insertTextFormat": FORMATO_SNIPPET,
                "textEdit": {"range": faixa(inicio), "newText": format!("{}$0", escapar_snippet(&e))}
            }));
        }
        // A sugestao da IA entra por ultimo e marcada; o Helix so a insere se a pessoa
        // aceitar. Sem configuracao, nada muda aqui. O `filterText` e a palavra digitada:
        // sem ele o Helix filtraria o item pelo rotulo «IA: ...» e ele sumiria da lista.
        if let Some(cfg) = &self.ia {
            let arquivo = caminho_de_uri(uri)
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| uri.to_string());
            if let Some(mut i) = ia::item(cfg, &arquivo, &doc.linguagem, &doc.texto, linha, coluna)
            {
                i["filterText"] = json!(palavra);
                i["textEdit"] = json!({"range": faixa(inicio), "newText": i["insertText"].clone()});
                itens.push(i);
            }
        }
        json!({"isIncomplete": false, "items": itens})
    }
}

/// A abreviacao antes do cursor: o trecho que termina na coluna (em caracteres; o LSP
/// conta UTF-16, que coincide fora do plano astral) e comeca depois do ultimo espaco,
/// aspa ou tag fechada -- espaco dentro de `{...}` e texto do Emmet e nao corta.
/// Devolve (inicio em caracteres, texto).
fn palavra_antes(linha: &str, coluna: usize) -> (usize, &str) {
    let chars: Vec<(usize, char)> = linha.char_indices().collect();
    let fim = chars.get(coluna).map(|(b, _)| *b).unwrap_or(linha.len());
    let antes = &linha[..fim];
    let mut inicio_b = 0;
    let mut chaves = 0usize;
    for (b, ch) in antes.char_indices().rev() {
        match ch {
            '}' => chaves += 1,
            '{' => chaves = chaves.saturating_sub(1),
            c if chaves == 0 && (c.is_whitespace() || c == '"' || c == '\'') => {
                inicio_b = b + c.len_utf8();
                break;
            }
            '<' if chaves == 0 => {
                // `<p>abc`: a abreviacao e `abc`, depois da tag; `<abc` sem fechar: `abc`.
                let depois = &antes[b + 1..];
                inicio_b = b + 1 + depois.find('>').map(|p| p + 1).unwrap_or(0);
                break;
            }
            _ => {}
        }
    }
    let inicio = antes[..inicio_b].chars().count();
    (inicio, &antes[inicio_b..])
}

fn caminho_de_uri(uri: &str) -> Option<PathBuf> {
    let resto = uri.strip_prefix("file://")?;
    // Percentual decodificado so no que aparece em caminho: espaco e afins.
    let mut s = String::new();
    let b = resto.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%'
            && i + 2 < b.len()
            && let Ok(h) = u8::from_str_radix(std::str::from_utf8(&b[i + 1..i + 3]).ok()?, 16)
        {
            s.push(h as char);
            i += 3;
            continue;
        }
        s.push(b[i] as char);
        i += 1;
    }
    Some(PathBuf::from(s))
}

/// Le uma mensagem com cabecalho `Content-Length` do fio. `Ok(None)` no fim do fio.
pub fn ler_mensagem(r: &mut impl BufRead) -> std::io::Result<Option<Value>> {
    let mut tamanho: Option<usize> = None;
    loop {
        let mut linha = String::new();
        if r.read_line(&mut linha)? == 0 {
            return Ok(None);
        }
        let l = linha.trim_end_matches(['\r', '\n']);
        if l.is_empty() {
            if tamanho.is_some() {
                break;
            }
            continue;
        }
        if let Some((k, v)) = l.split_once(':')
            && k.trim().eq_ignore_ascii_case("Content-Length")
        {
            tamanho = v.trim().parse().ok();
        }
    }
    let n = tamanho.ok_or_else(|| std::io::Error::other("sem Content-Length"))?;
    let mut corpo = vec![0u8; n];
    r.read_exact(&mut corpo)?;
    serde_json::from_slice(&corpo)
        .map(Some)
        .map_err(std::io::Error::other)
}

pub fn escrever_mensagem(w: &mut impl Write, v: &Value) -> std::io::Result<()> {
    let corpo = serde_json::to_vec(v)?;
    write!(w, "Content-Length: {}\r\n\r\n", corpo.len())?;
    w.write_all(&corpo)?;
    w.flush()
}

/// O laco do servidor: le ate o `exit` (ou o fim do fio), responde pelo mesmo fio.
pub fn servir(entrada: &mut impl BufRead, saida: &mut impl Write) -> std::io::Result<()> {
    let mut s = Servidor::default();
    while let Some(m) = ler_mensagem(entrada)? {
        if let Some(r) = s.tratar(&m) {
            escrever_mensagem(saida, &r)?;
        }
        if s.encerrar {
            break;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn palavra_antes_do_cursor_para_no_espaco_e_no_sinal_de_menor() {
        assert_eq!(palavra_antes("  div>ul", 8), (2, "div>ul"));
        assert_eq!(palavra_antes("<p>a.b", 6), (3, "a.b"));
        assert_eq!(palavra_antes("x p{a b}", 8), (2, "p{a b}"));
        assert_eq!(palavra_antes("<di", 3), (1, "di"));
        assert_eq!(palavra_antes("x", 0), (0, ""));
    }

    #[test]
    fn uri_vira_caminho_com_percentual() {
        assert_eq!(
            caminho_de_uri("file:///a/b%20c").unwrap(),
            PathBuf::from("/a/b c")
        );
    }

    #[test]
    fn completion_filtra_por_prefixo_e_linguagem_e_escapa_o_emmet() {
        let mut s = Servidor {
            snippets: vec![
                Snippet {
                    prefixo: "fnmain".into(),
                    corpo: "fn main() {\n    $0\n}".into(),
                    descricao: "main".into(),
                    linguagens: vec!["rust".into()],
                },
                Snippet {
                    prefixo: "fnx".into(),
                    corpo: "x".into(),
                    descricao: String::new(),
                    linguagens: vec!["python".into()],
                },
            ],
            ..Default::default()
        };
        s.tratar(
            &json!({"method":"textDocument/didOpen","params":{"textDocument":{
            "uri":"file:///a.rs","languageId":"rust","text":"fn\nfnm"}}}),
        );
        let r = s
            .tratar(&json!({"id":1,"method":"textDocument/completion","params":{
                "textDocument":{"uri":"file:///a.rs"},"position":{"line":1,"character":3}}}))
            .unwrap();
        let itens = r["result"]["items"].as_array().unwrap();
        assert_eq!(itens.len(), 1, "{r}");
        assert_eq!(itens[0]["label"], "fnmain");
        assert_eq!(itens[0]["insertTextFormat"], 2);
        assert_eq!(itens[0]["textEdit"]["range"]["start"]["character"], 0);
        // html: a barra do texto do usuario nao vira escape de tabstop (o `$` e numeracao
        // do proprio Emmet e nunca sobrevive ao expansor).
        s.tratar(
            &json!({"method":"textDocument/didOpen","params":{"textDocument":{
            "uri":"file:///a.html","languageId":"html","text":"p{a\\b 5}"}}}),
        );
        let r = s
            .tratar(&json!({"id":2,"method":"textDocument/completion","params":{
                "textDocument":{"uri":"file:///a.html"},"position":{"line":0,"character":8}}}))
            .unwrap();
        let t = r["result"]["items"][0]["textEdit"]["newText"]
            .as_str()
            .unwrap();
        assert_eq!(t, "<p>a\\\\b 5</p>$0");
    }

    /// Com um agente falso em loopback, o item «IA» aparece na lista do Helix ao lado dos
    /// snippets; sem configuracao, a mesma consulta nao tem o item e nao da erro. O RED:
    /// apagar o bloco `if let Some(cfg) = &self.ia` do `completion` derruba a primeira
    /// metade (medido).
    #[test]
    fn a_completion_traz_o_item_ia_so_quando_o_agente_esta_configurado() {
        use std::io::{BufRead as _, BufReader, Read as _, Write as _};
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let porta = l.local_addr().unwrap().port();
        std::thread::spawn(move || {
            let (mut s, _) = l.accept().unwrap();
            let mut r = BufReader::new(s.try_clone().unwrap());
            let mut n = 0usize;
            loop {
                let mut linha = String::new();
                r.read_line(&mut linha).unwrap();
                if let Some(v) = linha.to_ascii_lowercase().strip_prefix("content-length:") {
                    n = v.trim().parse().unwrap();
                }
                if linha == "\r\n" {
                    break;
                }
            }
            let mut corpo = vec![0; n];
            r.read_exact(&mut corpo).unwrap();
            let resposta = json!({"texto": "println!(\"ola\");", "modelo": "falso"}).to_string();
            write!(
                s,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n{}",
                resposta.len(),
                resposta
            )
            .unwrap();
        });
        let abrir = |sv: &mut Servidor| {
            sv.tratar(
                &json!({"method":"textDocument/didOpen","params":{"textDocument":{
                "uri":"file:///p/a.rs","languageId":"rust","text":"fn main() {\n    pr\n}"}}}),
            );
        };
        let pedir = |sv: &mut Servidor| {
            sv.tratar(&json!({"id":7,"method":"textDocument/completion","params":{
                "textDocument":{"uri":"file:///p/a.rs"},"position":{"line":1,"character":6}}}))
                .unwrap()
        };
        let mut com = Servidor {
            ia: Some(ia::Configuracao {
                host: "127.0.0.1".into(),
                porta,
                caminho: "/v1/ide/completar".into(),
                token: "t".into(),
            }),
            ..Default::default()
        };
        abrir(&mut com);
        let r = pedir(&mut com);
        let itens = r["result"]["items"].as_array().unwrap();
        let ia = itens.iter().find(|i| i["detail"] == "IA").expect("item IA");
        assert_eq!(ia["label"], "IA: println!(\"ola\");");
        assert_eq!(ia["preselect"], false);
        assert_eq!(ia["filterText"], "pr");
        assert_eq!(ia["textEdit"]["newText"], "println!(\"ola\");");

        let mut sem = Servidor::default();
        abrir(&mut sem);
        let r = pedir(&mut sem);
        assert!(r.get("error").is_none(), "{r}");
        assert!(
            r["result"]["items"]
                .as_array()
                .unwrap()
                .iter()
                .all(|i| i["detail"] != "IA")
        );
    }
}
