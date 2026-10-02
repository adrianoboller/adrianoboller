//! A completacao por IA: um item «IA» na lista do Helix, vindo do modelo configurado do
//! agente pela rota `POST /v1/ide/completar`. O servidor NAO fala com provedor nenhum: quem
//! sabe qual modelo, qual chave e qual teto e o agente, e este processo so pede a ele pela
//! rede local. Assim o snippet-ls continua sem dependencia (HTTP/1.1 de loopback escrito
//! aqui, sobre `TcpStream`) e a decisao de provedor fica num lugar so.
//!
//! O que a configuracao e: a URL da rota (chave `exportadas.ia_completar` do catalogo;
//! `http://` e loopback -- o token nao sai da maquina nem em claro pela rede) e o token da
//! API (`api.token`). O agente poe as duas no ambiente do Helix quando abre o IDE; sem elas
//! nao ha item e nao ha erro.
//!
//! O item nunca se aplica sozinho: `preselect: false`, ordenado por ultimo (`sortText`),
//! texto plano -- o Helix so insere o que a pessoa aceitar.

use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

/// Onde pedir e com que token. `None` quando o ambiente nao configura (ou configura errado).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Configuracao {
    pub host: String,
    pub porta: u16,
    pub caminho: String,
    pub token: String,
}

/// A chave do catalogo cujo valor e o token da API do agente.
const CHAVE_DO_TOKEN: &str = "api.token";

/// Teto de espera pela resposta: o Helix pede completacao a cada tecla, e um pedido que
/// pendura mais que isso vale menos que nenhum.
pub const PRAZO: Duration = Duration::from_secs(8);

/// Quantos caracteres antes e depois do cursor viajam ao agente.
const ANTES_MAX: usize = 4000;
const DEPOIS_MAX: usize = 1000;

/// `CompletionItemKind.Text`: um item de texto, sem a formatacao de snippet.
const KIND_TEXTO: u64 = 1;

/// Le a configuracao do ambiente. Recusa URL que nao seja `http://` em loopback, com o
/// motivo no stderr, porque o token iria em claro para onde a URL apontasse.
pub fn do_ambiente() -> Option<Configuracao> {
    // Os nomes vem do catalogo (`exportadas.ia_completar`, `api.token`): e o agente quem os
    // exporta ao Helix, e este processo so le o que ele exportou.
    let variavel =
        |chave: &str| phxclaw_config_runtime::agente::por_chave(chave).map(|c| c.variavel.clone());
    let url = std::env::var(variavel("exportadas.ia_completar")?).ok()?;
    let token = std::env::var(variavel(CHAVE_DO_TOKEN)?).ok()?;
    match analisar_url(&url) {
        Some((host, porta, caminho)) if token.trim().is_empty() => {
            let _ = (host, porta, caminho);
            eprintln!("phxclaw-snippet-ls: token da API vazio; completacao por IA desligada");
            None
        }
        Some((host, porta, caminho)) => Some(Configuracao {
            host,
            porta,
            caminho,
            token,
        }),
        None => {
            eprintln!(
                "phxclaw-snippet-ls: a URL da completacao precisa ser http://127.0.0.1:PORTA/... \
                 (loopback); completacao por IA desligada"
            );
            None
        }
    }
}

/// `http://127.0.0.1:8787/v1/ide/completar` -> (host, porta, caminho). So loopback.
pub fn analisar_url(url: &str) -> Option<(String, u16, String)> {
    let resto = url.strip_prefix("http://")?;
    let (autoridade, caminho) = match resto.find('/') {
        Some(i) => (&resto[..i], &resto[i..]),
        None => (resto, "/"),
    };
    let (host, porta) = autoridade.rsplit_once(':')?;
    let porta: u16 = porta.parse().ok()?;
    if !matches!(host, "127.0.0.1" | "localhost" | "[::1]") {
        return None;
    }
    Some((host.to_string(), porta, caminho.to_string()))
}

/// O item de completacao, ou `None` (sem resposta, resposta vazia, erro): silencio e o
/// comportamento certo de uma sugestao que nao veio.
pub fn item(
    cfg: &Configuracao,
    arquivo: &str,
    linguagem: &str,
    texto: &str,
    linha: usize,
    coluna: usize,
) -> Option<Value> {
    let (antes, depois) = em_volta_do_cursor(texto, linha, coluna);
    let pedido = json!({
        "arquivo": arquivo,
        "linguagem": linguagem,
        "antes": cauda(antes, ANTES_MAX),
        "depois": cabeca(depois, DEPOIS_MAX),
    });
    let resposta = pedir(cfg, &pedido.to_string()).ok()?;
    let sugestao = resposta["texto"].as_str()?.trim_end();
    if sugestao.trim().is_empty() {
        return None;
    }
    Some(item_de(sugestao, &resposta["modelo"]))
}

/// O item como o Helix o ve. `label` curto («IA: primeira linha»), o texto inteiro na
/// documentacao, e as tres marcas de «nunca sozinho»: preselect falso, ultimo na ordem,
/// texto plano.
pub fn item_de(sugestao: &str, modelo: &Value) -> Value {
    let primeira: String = sugestao
        .lines()
        .next()
        .unwrap_or("")
        .chars()
        .take(48)
        .collect();
    json!({
        "label": format!("IA: {primeira}"),
        "kind": KIND_TEXTO,
        "detail": "IA",
        "documentation": {"kind": "plaintext",
            "value": format!("{}\n\n[{}]", sugestao, modelo.as_str().unwrap_or("modelo"))},
        "sortText": "~~~ia",
        "preselect": false,
        "insertTextFormat": 1,
        "insertText": sugestao,
    })
}

/// O texto antes e depois do cursor (linha e coluna em caracteres, como o `completion`).
fn em_volta_do_cursor(texto: &str, linha: usize, coluna: usize) -> (&str, &str) {
    let mut corte = texto.len();
    let mut l = 0usize;
    let mut inicio_da_linha = 0usize;
    for (i, c) in texto.char_indices() {
        if l == linha {
            inicio_da_linha = i;
            break;
        }
        if c == '\n' {
            l += 1;
            inicio_da_linha = i + 1;
        }
    }
    if l == linha {
        let resto = &texto[inicio_da_linha..];
        let fim = resto
            .char_indices()
            .take_while(|(_, c)| *c != '\n')
            .nth(coluna)
            .map(|(i, _)| i)
            .unwrap_or_else(|| resto.find('\n').unwrap_or(resto.len()));
        corte = inicio_da_linha + fim;
    }
    texto.split_at(corte)
}

fn cauda(t: &str, n: usize) -> &str {
    let i = t
        .char_indices()
        .rev()
        .nth(n.saturating_sub(1))
        .map(|(i, _)| i)
        .unwrap_or(0);
    &t[i..]
}

fn cabeca(t: &str, n: usize) -> &str {
    let i = t.char_indices().nth(n).map(|(i, _)| i).unwrap_or(t.len());
    &t[..i]
}

/// Um POST HTTP/1.1 de loopback, sem biblioteca: escreve o pedido, le o estado, os
/// cabecalhos e o corpo por `Content-Length` (ou ate o fim). Qualquer estado fora de 2xx e
/// `Err`, que o chamador traduz em «sem item».
pub fn pedir(cfg: &Configuracao, corpo: &str) -> Result<Value, String> {
    let endereco = (cfg.host.as_str(), cfg.porta)
        .to_socket_addrs()
        .map_err(|e| e.to_string())?
        .next()
        .ok_or("endereco nao resolve")?;
    let mut s = TcpStream::connect_timeout(&endereco, PRAZO).map_err(|e| e.to_string())?;
    s.set_read_timeout(Some(PRAZO)).map_err(|e| e.to_string())?;
    s.set_write_timeout(Some(PRAZO))
        .map_err(|e| e.to_string())?;
    let pedido = format!(
        "POST {} HTTP/1.1\r\nHost: {}:{}\r\nAuthorization: Bearer {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        cfg.caminho,
        cfg.host,
        cfg.porta,
        cfg.token,
        corpo.len(),
        corpo
    );
    s.write_all(pedido.as_bytes()).map_err(|e| e.to_string())?;
    let mut r = BufReader::new(s);
    let mut estado = String::new();
    r.read_line(&mut estado).map_err(|e| e.to_string())?;
    let codigo: u16 = estado
        .split_whitespace()
        .nth(1)
        .and_then(|c| c.parse().ok())
        .ok_or_else(|| format!("resposta sem estado: {estado:?}"))?;
    let mut tamanho: Option<usize> = None;
    loop {
        let mut l = String::new();
        if r.read_line(&mut l).map_err(|e| e.to_string())? == 0 || l == "\r\n" || l == "\n" {
            break;
        }
        if let Some((k, v)) = l.split_once(':')
            && k.eq_ignore_ascii_case("content-length")
        {
            tamanho = v.trim().parse().ok();
        }
    }
    let mut corpo = Vec::new();
    match tamanho {
        Some(n) => {
            corpo.resize(n, 0);
            r.read_exact(&mut corpo).map_err(|e| e.to_string())?;
        }
        None => {
            r.read_to_end(&mut corpo).map_err(|e| e.to_string())?;
        }
    }
    if !(200..300).contains(&codigo) {
        return Err(format!("HTTP {codigo}"));
    }
    serde_json::from_slice(&corpo).map_err(|e| e.to_string())
}

#[cfg(test)]
mod testes {
    use super::*;
    use std::net::TcpListener;

    #[test]
    fn so_url_http_de_loopback_configura() {
        assert_eq!(
            analisar_url("http://127.0.0.1:8787/v1/ide/completar"),
            Some(("127.0.0.1".into(), 8787, "/v1/ide/completar".into()))
        );
        assert_eq!(
            analisar_url("http://localhost:1"),
            Some(("localhost".into(), 1, "/".into()))
        );
        // https (nao e loopback em claro), host de fora, sem porta: nenhum configura.
        assert_eq!(analisar_url("https://127.0.0.1:8787/x"), None);
        assert_eq!(analisar_url("http://10.0.0.5:8787/x"), None);
        assert_eq!(analisar_url("http://127.0.0.1/x"), None);
    }

    #[test]
    fn o_texto_se_parte_no_cursor() {
        let t = "ab\ncd\nef";
        assert_eq!(em_volta_do_cursor(t, 1, 1), ("ab\nc", "d\nef"));
        assert_eq!(em_volta_do_cursor(t, 0, 0), ("", t));
        assert_eq!(em_volta_do_cursor(t, 2, 9), (t, ""));
        assert_eq!(em_volta_do_cursor(t, 7, 0), (t, ""));
    }

    #[test]
    fn o_item_ia_nunca_vem_preselecionado_e_fica_por_ultimo() {
        let i = item_de("let x = 1;\nlet y = 2;", &json!("falso"));
        assert_eq!(i["label"], "IA: let x = 1;");
        assert_eq!(i["detail"], "IA");
        assert_eq!(i["preselect"], false);
        assert_eq!(i["insertTextFormat"], 1);
        assert_eq!(i["insertText"], "let x = 1;\nlet y = 2;");
        assert!(i["sortText"].as_str().unwrap().starts_with('~'));
    }

    /// Um agente falso em loopback responde a completacao; o cliente HTTP escrito aqui le
    /// o estado, os cabecalhos e o corpo, e o item chega com o rotulo IA.
    #[test]
    fn com_um_agente_falso_a_completacao_chega_com_o_rotulo_ia() {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let porta = l.local_addr().unwrap().port();
        let servidor = std::thread::spawn(move || {
            let (s, _) = l.accept().unwrap();
            let mut r = BufReader::new(s);
            let mut cabecalhos = Vec::new();
            let mut tamanho = 0usize;
            loop {
                let mut linha = String::new();
                r.read_line(&mut linha).unwrap();
                if linha == "\r\n" {
                    break;
                }
                if let Some(v) = linha.to_ascii_lowercase().strip_prefix("content-length:") {
                    tamanho = v.trim().parse().unwrap();
                }
                cabecalhos.push(linha);
            }
            let mut corpo = vec![0; tamanho];
            r.read_exact(&mut corpo).unwrap();
            let pedido: Value = serde_json::from_slice(&corpo).unwrap();
            let resposta = json!({"texto": "```rust\nreturn 42;\n```".replace("```rust\n", "").replace("\n```", ""), "modelo": "falso"}).to_string();
            let mut s = r.into_inner();
            write!(
                s,
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                resposta.len(),
                resposta
            )
            .unwrap();
            (cabecalhos, pedido)
        });
        let cfg = Configuracao {
            host: "127.0.0.1".into(),
            porta,
            caminho: "/v1/ide/completar".into(),
            token: "tok".into(),
        };
        let i = item(
            &cfg,
            "src/main.rs",
            "rust",
            "fn f() -> i32 {\n    \n}",
            1,
            4,
        )
        .unwrap();
        assert_eq!(i["label"], "IA: return 42;");
        assert_eq!(i["detail"], "IA");
        let (cabecalhos, pedido) = servidor.join().unwrap();
        assert!(
            cabecalhos
                .iter()
                .any(|c| c.trim() == "Authorization: Bearer tok"),
            "{cabecalhos:?}"
        );
        assert_eq!(pedido["antes"], "fn f() -> i32 {\n    ");
        assert_eq!(pedido["depois"], "\n}");
        assert_eq!(pedido["linguagem"], "rust");
    }

    #[test]
    fn agente_que_recusa_ou_responde_vazio_nao_vira_item() {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let porta = l.local_addr().unwrap().port();
        std::thread::spawn(move || {
            for resposta in [
                "HTTP/1.1 503 x\r\nContent-Length: 2\r\n\r\n{}",
                "HTTP/1.1 200 OK\r\nContent-Length: 13\r\n\r\n{\"texto\":\"  \"}",
            ] {
                let (mut s, _) = l.accept().unwrap();
                let mut r = BufReader::new(s.try_clone().unwrap());
                loop {
                    let mut linha = String::new();
                    r.read_line(&mut linha).unwrap();
                    if linha == "\r\n" || linha.is_empty() {
                        break;
                    }
                }
                s.write_all(resposta.as_bytes()).unwrap();
            }
        });
        let cfg = Configuracao {
            host: "127.0.0.1".into(),
            porta,
            caminho: "/x".into(),
            token: "t".into(),
        };
        assert!(item(&cfg, "a.rs", "rust", "x", 0, 1).is_none());
        assert!(item(&cfg, "a.rs", "rust", "x", 0, 1).is_none());
    }
}
