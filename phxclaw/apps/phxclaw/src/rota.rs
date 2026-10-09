//! `phxclaw api METODO ROTA` e `phxclaw api rotas`: toda rota do `servir` pela linha de
//! comando, pelo cliente do SDK e com o token local.
//!
//! Por que um cliente e nao a funcao do handler chamada aqui dentro: a rota mora num
//! servidor com estado (tarefas rodando, agenda, sessoes do IDE). Montar um segundo
//! estado neste processo daria uma segunda API que nao ve as tarefas da primeira -- e a
//! tarefa criada morreria com o comando. O pedido vai ao servidor de pe e quem responde e
//! o MESMO handler, atras do MESMO portao do RBAC; esta porta so monta o pedido e mostra a
//! resposta. Onde existe comando local que chama as mesmas funcoes sem servidor, a tabela
//! diz qual (`rotas::Rota::equivalente`).

use anyhow::{Result, bail};
use phxclaw_agent::rotas::{self, Forma, ROTAS};
use serde_json::Value;
use std::io::Read;
use std::path::Path;

pub const METODOS: &[&str] = &["GET", "POST", "PUT", "DELETE", "PATCH"];

/// A tabela das rotas, como `api rotas` a imprime.
pub fn tabela(cli: &str) -> String {
    let mut s = format!(
        "{} rotas do `{cli} servir`. Chame qualquer uma com `{cli} api METODO ROTA`.\n\n",
        ROTAS.len()
    );
    let largura = ROTAS
        .iter()
        .map(|r| r.metodo.len() + 1 + r.caminho.len())
        .max()
        .unwrap_or(0);
    let recuo = " ".repeat(largura + 6);
    let largura_da_tela = crate::ajuda::LARGURA;
    for r in ROTAS {
        let alvo = format!("{} {}", r.metodo, r.caminho);
        s.push_str(&format!("  {alvo:<largura$}  {}\n", r.resumo));
        let mut nota = |t: String| s.push_str(&crate::ajuda::quebrar(&t, &recuo, largura_da_tela));
        if !r.parametros.is_empty() {
            nota(r.parametros.to_string());
        }
        match (r.forma, r.equivalente) {
            (Forma::WebSocket(m), _) => nota(format!("websocket: {m}")),
            (Forma::Tela, _) => nota("tela (navegador)".into()),
            (Forma::Http, Some(e)) => nota(format!("local: {cli} {e}")),
            (Forma::Http, None) => {}
        }
    }
    s
}

pub fn em_json() -> Value {
    Value::Array(
        ROTAS
            .iter()
            .map(|r| {
                serde_json::json!({
                    "metodo": r.metodo,
                    "caminho": r.caminho,
                    "resumo": r.resumo,
                    "parametros": r.parametros,
                    "equivalente": r.equivalente,
                    "forma": match r.forma {
                        Forma::Http => "http",
                        Forma::Tela => "tela",
                        Forma::WebSocket(_) => "websocket",
                    },
                })
            })
            .collect(),
    )
}

/// O pedido montado das opcoes: caminho com a consulta, corpo e cabecalhos.
#[derive(Debug, PartialEq)]
pub struct Pedido {
    pub metodo: String,
    pub caminho: String,
    pub corpo: Option<Value>,
    pub projeto: Option<String>,
}

/// `METODO ROTA [--corpo ARQ|-|JSON] [--consulta K=V]... [--projeto P]`.
pub fn pedido(args: &[String], entrada: &mut dyn Read) -> Result<Pedido, String> {
    let metodo = args
        .first()
        .map(|m| m.to_ascii_uppercase())
        .unwrap_or_default();
    if !METODOS.contains(&metodo.as_str()) {
        return Err(format!("metodo {metodo:?}: use {}", METODOS.join("|")));
    }
    let Some(caminho) = args.get(1).filter(|c| c.starts_with('/')) else {
        return Err("falta a ROTA (comeca por /); `api rotas` lista".into());
    };
    let Some(r) = rotas::achar(&metodo, caminho) else {
        let outros: Vec<&str> = ROTAS
            .iter()
            .filter(|r| rotas::casa(r.caminho, caminho.split('?').next().unwrap_or("")))
            .map(|r| r.metodo)
            .collect();
        return Err(if outros.is_empty() {
            format!(
                "{metodo} {caminho}: rota desconhecida; `api rotas` lista as {}",
                ROTAS.len()
            )
        } else {
            format!(
                "{metodo} {caminho}: a rota existe so com {}",
                outros.join(", ")
            )
        });
    };
    if let Forma::WebSocket(m) = r.forma {
        return Err(format!("{} {}: websocket ({m})", r.metodo, r.caminho));
    }
    let mut caminho = caminho.clone();
    let mut corpo = None;
    let mut projeto = None;
    let mut i = 2;
    while i < args.len() {
        let valor = || {
            args.get(i + 1)
                .cloned()
                .ok_or_else(|| format!("{} pede um valor", args[i]))
        };
        match args[i].as_str() {
            "--corpo" => {
                let v = valor()?;
                let texto = if v == "-" {
                    let mut t = String::new();
                    entrada.read_to_string(&mut t).map_err(|e| e.to_string())?;
                    t
                } else if v.trim_start().starts_with(['{', '[']) {
                    // O JSON em linha esta no argv: o mesmo crivo do `ferramenta`.
                    crate::ferramenta::recusar_credencial_no_argv("--corpo", &v, "--corpo -")?;
                    v
                } else {
                    std::fs::read_to_string(&v).map_err(|e| format!("--corpo {v}: {e}"))?
                };
                corpo = Some(serde_json::from_str(&texto).map_err(|e| format!("--corpo: {e}"))?);
            }
            "--consulta" => {
                let v = valor()?;
                let Some((k, x)) = v.split_once('=') else {
                    return Err(format!("--consulta {v}: use CHAVE=VALOR"));
                };
                crate::ferramenta::recusar_credencial_no_argv("--consulta", x, "--corpo -")?;
                caminho.push(if caminho.contains('?') { '&' } else { '?' });
                caminho.push_str(&format!("{}={}", codificar(k), codificar(x)));
            }
            "--projeto" => projeto = Some(valor()?),
            // Interruptor sem valor, lido pelo `comando` (`destino`).
            OPCAO_CONFIO => {
                i += 1;
                continue;
            }
            // Do comando, lidas por quem chamou.
            "--url" | "--saida" | "--pasta" => {}
            outro => return Err(format!("opcao desconhecida: {outro}")),
        }
        i += 2;
    }
    Ok(Pedido {
        metodo,
        caminho,
        corpo,
        projeto,
    })
}

/// O interruptor que aceita mandar o Bearer a um `--url` fora de loopback.
pub const OPCAO_CONFIO: &str = "--confio-nesta-url";

/// O servidor padrao do `api`, quando nem `--url` nem `api.url_cliente` dizem.
pub const URL_PADRAO: &str = "http://127.0.0.1:8787";

/// O host e loopback? So pelo texto (nome `localhost` ou IP de loopback): resolver o nome
/// para decidir abriria a porta do DNS que aponta `meu.host` para 127.0.0.1 hoje e para
/// fora amanha. `[::ffff:127.0.0.1]` e `0.0.0.0` NAO sao loopback aqui, de proposito: o lado
/// estrito e o que nao vaza.
fn host_de_loopback(host: &str) -> bool {
    let h = host.trim_start_matches('[').trim_end_matches(']');
    h.eq_ignore_ascii_case("localhost")
        || h.parse::<std::net::IpAddr>()
            .is_ok_and(|ip| ip.is_loopback())
}

/// Pode o Bearer sair para `base`? O token do `api` alcanca `/v1/tunel/terminal` (um shell
/// no servidor), entao a decisao vem ANTES de montar o cliente -- nada sai do processo
/// quando ela recusa:
///
/// - `http` fora de loopback: recusado sempre. O Bearer iria em claro pela rede, e nenhum
///   interruptor torna isso aceitavel.
/// - `--url` digitado fora de loopback: so com `--confio-nesta-url`. O `--url` vem de
///   historico, roteiro e documentacao copiada; o interruptor faz quem roda dizer que o
///   servidor e dele.
/// - `api.url_cliente` (ambiente, perfil ou pasta: o catalogo nao a aceita do projeto) em
///   `https`: vale, e a escolha do operador.
pub fn destino(base: &str, explicita: bool, confio: bool) -> Result<(), String> {
    let uri: axum::http::Uri = base
        .parse()
        .map_err(|e| format!("{base}: URL invalida ({e})"))?;
    let esquema = uri.scheme_str().unwrap_or("");
    let Some(host) = uri.host().filter(|h| !h.is_empty()) else {
        return Err(format!("{base}: URL sem host"));
    };
    let local = host_de_loopback(host);
    match esquema {
        "http" | "https" => {}
        outro => {
            return Err(format!(
                "{base}: esquema {outro:?}; use http (loopback) ou https"
            ));
        }
    }
    if esquema == "http" && !local {
        return Err(format!(
            "{base}: http fora de loopback, o token da API iria em claro pela rede (e ele alcanca \
             o terminal do servidor); use https, ou http em 127.0.0.1/localhost. Nada foi enviado."
        ));
    }
    if explicita && !local && !confio {
        return Err(format!(
            "--url {base}: host {host} fora de loopback; o token da API iria para ele. Se o \
             servidor e seu, repita com {OPCAO_CONFIO}. Nada foi enviado."
        ));
    }
    Ok(())
}

/// Escape de componente de consulta (RFC 3986, nao reservados passam).
fn codificar(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}

/// O comando inteiro (sem o `chave`, que o `main` despacha antes). Devolve o codigo de
/// saida: 0 para 2xx, 1 para o resto.
pub async fn comando(args: &[String], token: Option<String>, cli: &str) -> Result<i32> {
    if args.first().map(String::as_str) == Some("rotas") {
        if args.iter().any(|a| a == "--json") {
            println!("{}", serde_json::to_string_pretty(&em_json())?);
        } else {
            print!("{}", tabela(cli));
        }
        return Ok(0);
    }
    let p = pedido(args, &mut std::io::stdin().lock()).map_err(anyhow::Error::msg)?;
    let Some(token) = token else {
        bail!(
            "sem token da API: o `{cli} servir` grava <pasta>/api.token na primeira vez; ou \
             PHXCLAW_API_TOKEN, ou `{cli} api chave`"
        );
    };
    let explicita = super::opcao(args, "--url");
    let base = explicita
        .clone()
        .or_else(|| phxclaw_agent::config::texto_de("api.url_cliente"))
        .unwrap_or_else(|| URL_PADRAO.into());
    destino(
        &base,
        explicita.is_some(),
        args.iter().any(|a| a == OPCAO_CONFIO),
    )
    .map_err(anyhow::Error::msg)?;
    let c = phxclaw_sdk::Cliente::new(&base, token).map_err(|e| anyhow::anyhow!("{e}"))?;
    let cab: Vec<(&str, &str)> = p
        .projeto
        .as_deref()
        .map(|v| ("X-PhxClaw-Projeto", v))
        .into_iter()
        .collect();
    let r = c
        .rota(&p.metodo, &p.caminho, p.corpo.as_ref(), &cab)
        .await
        .map_err(|e| anyhow::anyhow!("{base}: {e}"))?;
    if let Some(arq) = super::opcao(args, "--saida") {
        std::fs::write(Path::new(&arq), &r.corpo)?;
        eprintln!("{} bytes em {arq}", r.corpo.len());
    } else if r.tipo.starts_with("application/json")
        && let Ok(v) = serde_json::from_slice::<Value>(&r.corpo)
    {
        println!("{}", serde_json::to_string_pretty(&v)?);
    } else {
        use std::io::Write;
        std::io::stdout().write_all(&r.corpo)?;
    }
    eprintln!("HTTP {}", r.status);
    Ok(if (200..300).contains(&r.status) { 0 } else { 1 })
}

#[cfg(test)]
mod testes {
    use super::*;

    fn a(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn monta_o_pedido_com_consulta_corpo_e_projeto() {
        let p = pedido(
            &a(&[
                "get",
                "/v1/ide/arquivo",
                "--consulta",
                "caminho=src/a b.rs",
                "--projeto",
                "x",
            ]),
            &mut std::io::empty(),
        )
        .unwrap();
        assert_eq!(p.metodo, "GET");
        assert_eq!(p.caminho, "/v1/ide/arquivo?caminho=src%2Fa%20b.rs");
        assert_eq!(p.projeto.as_deref(), Some("x"));
        let p = pedido(
            &a(&["POST", "/v1/tasks/7/answer", "--corpo", "-"]),
            &mut "{\"answer\":\"sim\"}".as_bytes(),
        )
        .unwrap();
        assert_eq!(p.corpo, Some(serde_json::json!({"answer":"sim"})));
    }

    #[test]
    fn recusa_rota_desconhecida_metodo_errado_e_websocket() {
        let e = pedido(&a(&["GET", "/v1/nada"]), &mut std::io::empty()).unwrap_err();
        assert!(e.contains("desconhecida"), "{e}");
        let e = pedido(&a(&["DELETE", "/v1/tasks"]), &mut std::io::empty()).unwrap_err();
        assert!(e.contains("GET") && e.contains("POST"), "{e}");
        let e = pedido(&a(&["GET", "/v1/ide/terminal"]), &mut std::io::empty()).unwrap_err();
        assert!(e.contains("websocket"), "{e}");
    }

    /// O Bearer so sai para loopback ou https; `--url` de fora pede o interruptor. RED
    /// medido com `destino` reduzido a `Ok(())` (defeito reposto): falha na primeira recusa.
    #[test]
    fn o_token_so_sai_para_loopback_ou_https_e_url_de_fora_pede_confianca() {
        for ok in [
            "http://127.0.0.1:8787",
            "http://localhost:9",
            "http://[::1]:8787",
            "https://127.0.0.1",
        ] {
            assert_eq!(destino(ok, true, false), Ok(()), "{ok}");
            assert_eq!(destino(ok, false, false), Ok(()), "{ok}");
        }
        // http fora de loopback: nunca, nem com o interruptor, nem vindo da configuracao.
        for fora in [
            "http://atacante.exemplo",
            "http://10.0.0.5:8787",
            "http://0.0.0.0:8787",
            "http://[::ffff:127.0.0.1]:8787",
            "http://localhost.atacante.exemplo",
        ] {
            for (explicita, confio) in [(false, false), (true, false), (true, true)] {
                let e = destino(fora, explicita, confio).unwrap_err();
                assert!(
                    e.contains("em claro") && e.contains("Nada foi enviado"),
                    "{e}"
                );
            }
        }
        // https de fora: da configuracao vale; do --url, so com o interruptor.
        let s = "https://agente.exemplo";
        assert_eq!(destino(s, false, false), Ok(()));
        let e = destino(s, true, false).unwrap_err();
        assert!(e.contains(OPCAO_CONFIO), "{e}");
        assert_eq!(destino(s, true, true), Ok(()));
        assert!(destino("ftp://127.0.0.1", false, false).is_err());
        // O interruptor nao tem valor: nao engole a opcao seguinte.
        let p = pedido(
            &a(&["GET", "/v1/tasks", OPCAO_CONFIO, "--projeto", "x"]),
            &mut std::io::empty(),
        )
        .unwrap();
        assert_eq!(p.projeto.as_deref(), Some("x"));
    }

    /// Credencial no argv (`--corpo` em linha, `--consulta`) fica em `ps` e no historico:
    /// recusada, apontando a entrada padrao. O arquivo e o `-` continuam valendo.
    #[test]
    fn credencial_no_argv_do_api_e_recusada() {
        let chave = "ghp_0123456789abcdefghijABCDEFGHIJ0123";
        let e = pedido(
            &a(&[
                "POST",
                "/v1/tasks",
                "--corpo",
                &format!("{{\"t\":\"{chave}\"}}"),
            ]),
            &mut std::io::empty(),
        )
        .unwrap_err();
        assert!(e.contains("--corpo -") && !e.contains(chave), "{e}");
        let e = pedido(
            &a(&["GET", "/v1/tasks", "--consulta", &format!("t={chave}")]),
            &mut std::io::empty(),
        )
        .unwrap_err();
        assert!(e.contains("--consulta") && !e.contains(chave), "{e}");
        let p = pedido(
            &a(&["POST", "/v1/tasks", "--corpo", "-"]),
            &mut format!("{{\"t\":\"{chave}\"}}").as_bytes(),
        )
        .unwrap();
        assert!(p.corpo.is_some());
    }

    #[test]
    fn toda_rota_aparece_na_tabela_impressa() {
        let t = tabela("phxclaw");
        // A tabela quebra as linhas longas: confere-se o texto sem as quebras.
        let plano = t.split_whitespace().collect::<Vec<_>>().join(" ");
        assert!(t.lines().all(|l| l.chars().count() <= 110), "{t}");
        for r in ROTAS {
            assert!(
                t.contains(&format!("{} {}", r.metodo, r.caminho)),
                "{} {}",
                r.metodo,
                r.caminho
            );
            if !r.parametros.is_empty() {
                let p = r
                    .parametros
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .join(" ");
                assert!(plano.contains(&p), "{p}");
            }
        }
    }
}
