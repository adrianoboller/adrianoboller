//! Caixa de entrada em disco, para os servicos que nao deixam perguntar "o que veio desde
//! X": os que empurram (webhook) e os que entregam uma vez so (signal-cli, IRC, XMPP).
//!
//! O problema que ela resolve: o laco grava o cursor DEPOIS da tarefa para nao perder
//! mensagem quando cai no meio. Num webhook, o servico considera entregue quando recebe o
//! 200; se o 200 saisse antes de a mensagem estar em disco, uma queda entre os dois
//! perderia a mensagem sem ninguem saber. Entao o webhook so responde 200 depois de a
//! linha estar no arquivo com `fsync`, e o laco le da caixa pelo cursor como le de qualquer
//! outro servico -- a regra do cursor fica uma so.

use super::{Entrada, Mensagem};
use axum::Router;
use axum::body::Bytes;
use axum::extract::{Path as Caminho, RawQuery, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, HashSet};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Linha {
    seq: u64,
    conversa: String,
    autor: String,
    id: String,
    #[serde(default)]
    texto: Option<String>,
    /// O que o provedor precisa para responder e nao e mensagem (o `serviceUrl` do Teams).
    #[serde(default)]
    extra: Value,
}

struct Estado {
    linhas: Vec<Linha>,
    ids: HashSet<(String, String)>,
}

pub struct Caixa {
    arq: PathBuf,
    estado: Mutex<Estado>,
    chegou: Condvar,
}

impl Caixa {
    pub fn abrir(arq: impl AsRef<Path>) -> Result<Arc<Self>, String> {
        let arq = arq.as_ref().to_path_buf();
        if let Some(p) = arq.parent() {
            std::fs::create_dir_all(p).map_err(|e| e.to_string())?;
        }
        let mut linhas = Vec::new();
        if let Ok(t) = std::fs::read_to_string(&arq) {
            for l in t.lines().filter(|l| !l.trim().is_empty()) {
                // Linha cortada no fim (queda no meio da escrita) nunca recebeu 200: o
                // servico reenvia, e pular a linha e o certo.
                if let Ok(x) = serde_json::from_str::<Linha>(l) {
                    linhas.push(x);
                }
            }
        }
        let ids = linhas
            .iter()
            .map(|l| (l.conversa.clone(), l.id.clone()))
            .collect();
        Ok(Arc::new(Self {
            arq,
            estado: Mutex::new(Estado { linhas, ids }),
            chegou: Condvar::new(),
        }))
    }

    /// Grava as mensagens novas (as repetidas -- o servico reenvia o webhook -- ficam de
    /// fora) e so volta depois do `fsync`. Mensagem sem id ganha o numero da linha.
    pub fn anexar(&self, msgs: Vec<(Mensagem, Value)>) -> Result<usize, String> {
        let mut g = self
            .estado
            .lock()
            .map_err(|_| "caixa envenenada".to_string())?;
        let mut novas = Vec::new();
        let mut seq = g.linhas.last().map_or(0, |l| l.seq);
        for (m, extra) in msgs {
            seq += 1;
            let id = if m.id.is_empty() {
                format!("cx{seq}")
            } else {
                m.id
            };
            let chave = (m.conversa.clone(), id.clone());
            if g.ids.contains(&chave)
                || novas
                    .iter()
                    .any(|l: &Linha| (l.conversa.clone(), l.id.clone()) == chave)
            {
                seq -= 1;
                continue;
            }
            novas.push(Linha {
                seq,
                conversa: m.conversa,
                autor: m.autor,
                id,
                texto: m.texto,
                extra,
            });
        }
        if novas.is_empty() {
            return Ok(0);
        }
        let mut f = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.arq)
            .map_err(|e| e.to_string())?;
        let mut buf = String::new();
        for l in &novas {
            buf.push_str(&serde_json::to_string(l).map_err(|e| e.to_string())?);
            buf.push('\n');
        }
        f.write_all(buf.as_bytes())
            .and_then(|()| f.sync_data())
            .map_err(|e| format!("caixa nao gravou: {e}"))?;
        let n = novas.len();
        for l in novas {
            g.ids.insert((l.conversa.clone(), l.id.clone()));
            g.linhas.push(l);
        }
        drop(g);
        self.chegou.notify_all();
        Ok(n)
    }

    /// O lote desde `cursor` (o proximo `seq` esperado; None e "desde o comeco": a caixa so
    /// tem o que chegou para este canal). Vazio, espera ate `espera` por uma chegada.
    pub fn ler_desde(
        &self,
        cursor: Option<&str>,
        espera: Duration,
    ) -> Result<Vec<Entrada>, String> {
        let desde: u64 = cursor.and_then(|c| c.parse().ok()).unwrap_or(1);
        let fim = Instant::now() + espera;
        let mut g = self
            .estado
            .lock()
            .map_err(|_| "caixa envenenada".to_string())?;
        loop {
            let lote: Vec<Entrada> = g
                .linhas
                .iter()
                .filter(|l| l.seq >= desde)
                .map(|l| Entrada {
                    cursor: Some((l.seq + 1).to_string()),
                    mensagem: Some(Mensagem {
                        conversa: l.conversa.clone(),
                        autor: l.autor.clone(),
                        id: l.id.clone(),
                        texto: l.texto.clone(),
                    }),
                })
                .collect();
            let agora = Instant::now();
            if !lote.is_empty() || agora >= fim {
                return Ok(lote);
            }
            g = self
                .chegou
                .wait_timeout(g, fim - agora)
                .map_err(|_| "caixa envenenada".to_string())?
                .0;
        }
    }

    /// O `extra` da ultima mensagem da conversa.
    pub fn extra_de(&self, conversa: &str) -> Option<Value> {
        let g = self.estado.lock().ok()?;
        g.linhas
            .iter()
            .rev()
            .find(|l| l.conversa == conversa && !l.extra.is_null())
            .map(|l| l.extra.clone())
    }

    /// Linhas de uma conversa a partir de `desde` (o webchat le a caixa de SAIDA assim).
    pub fn da_conversa(&self, conversa: &str, desde: u64) -> Vec<(u64, String)> {
        let Ok(g) = self.estado.lock() else {
            return vec![];
        };
        g.linhas
            .iter()
            .filter(|l| l.conversa == conversa && l.seq >= desde)
            .map(|l| (l.seq, l.texto.clone().unwrap_or_default()))
            .collect()
    }
}

/// O pedido de webhook como o provedor precisa ver: cabecalhos em minusculas, a consulta
/// crua e o corpo em bytes -- a assinatura e sobre os bytes, nao sobre o JSON reanalisado.
pub struct PedidoWebhook {
    pub cabecalhos: BTreeMap<String, String>,
    pub consulta: String,
    pub corpo: Vec<u8>,
}

impl PedidoWebhook {
    pub fn cabecalho(&self, nome: &str) -> Option<&str> {
        self.cabecalhos
            .get(&nome.to_ascii_lowercase())
            .map(String::as_str)
    }

    pub fn parametro(&self, nome: &str) -> Option<String> {
        parametros(&self.consulta)
            .into_iter()
            .find(|(k, _)| k == nome)
            .map(|(_, v)| v)
    }
}

/// Pares `chave=valor` de uma consulta ou de um corpo de formulario, decodificados.
pub fn parametros(texto: &str) -> Vec<(String, String)> {
    reqwest::Url::parse(&format!("http://x/?{texto}"))
        .map(|u| {
            u.query_pairs()
                .map(|(k, v)| (k.into_owned(), v.into_owned()))
                .collect()
        })
        .unwrap_or_default()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RespostaWebhook {
    pub status: u16,
    pub tipo: &'static str,
    pub corpo: String,
}

impl RespostaWebhook {
    pub fn ok() -> Self {
        Self {
            status: 200,
            tipo: "application/json",
            corpo: "{}".into(),
        }
    }

    pub fn texto(corpo: impl Into<String>) -> Self {
        Self {
            status: 200,
            tipo: "text/plain",
            corpo: corpo.into(),
        }
    }

    pub fn json(v: Value) -> Self {
        Self {
            status: 200,
            tipo: "application/json",
            corpo: v.to_string(),
        }
    }
}

pub type Recusa = (u16, String);

/// O lado de entrada de um provedor de webhook. `post` confere a assinatura, extrai as
/// mensagens e grava na caixa -- nessa ordem, e a gravacao antes da resposta.
pub trait Recebedor: Send + Sync + 'static {
    fn nome(&self) -> &str;
    fn post(&self, p: &PedidoWebhook) -> Result<RespostaWebhook, Recusa>;
    /// O aperto de mao de quem confere a URL por GET (Meta: `hub.challenge`).
    fn get(&self, _p: &PedidoWebhook) -> Result<RespostaWebhook, Recusa> {
        Err((405, "metodo nao aceito".into()))
    }
}

type Mapa = Arc<BTreeMap<String, Arc<dyn Recebedor>>>;

/// `POST|GET /canais/{nome}/webhook`. O corpo passa de 1 MiB e recusa antes de qualquer
/// conta de assinatura.
pub fn rotas(recebedores: Vec<Arc<dyn Recebedor>>) -> Router {
    let mapa: Mapa = Arc::new(
        recebedores
            .into_iter()
            .map(|r| (r.nome().to_string(), r))
            .collect(),
    );
    Router::new()
        .route(
            "/canais/{nome}/webhook",
            post(receber_post).get(receber_get),
        )
        .layer(axum::extract::DefaultBodyLimit::max(1 << 20))
        .with_state(mapa)
}

fn pedido(h: &HeaderMap, consulta: Option<String>, corpo: Bytes) -> PedidoWebhook {
    PedidoWebhook {
        cabecalhos: h
            .iter()
            .filter_map(|(k, v)| {
                Some((
                    k.as_str().to_ascii_lowercase(),
                    v.to_str().ok()?.to_string(),
                ))
            })
            .collect(),
        consulta: consulta.unwrap_or_default(),
        corpo: corpo.to_vec(),
    }
}

async fn despachar(mapa: Mapa, nome: String, p: PedidoWebhook, get: bool) -> Response {
    let Some(r) = mapa.get(&nome).cloned() else {
        return (StatusCode::NOT_FOUND, "canal desconhecido").into_response();
    };
    // Conferir assinatura le o segredo do broker e gravar faz fsync: bloqueante.
    let r = tokio::task::spawn_blocking(move || if get { r.get(&p) } else { r.post(&p) }).await;
    match r {
        Ok(Ok(x)) => (
            StatusCode::from_u16(x.status).unwrap_or(StatusCode::OK),
            [(header::CONTENT_TYPE, x.tipo)],
            x.corpo,
        )
            .into_response(),
        Ok(Err((s, e))) => (
            StatusCode::from_u16(s).unwrap_or(StatusCode::BAD_REQUEST),
            e,
        )
            .into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

async fn receber_post(
    State(m): State<Mapa>,
    Caminho(nome): Caminho<String>,
    RawQuery(q): RawQuery,
    h: HeaderMap,
    corpo: Bytes,
) -> Response {
    despachar(m, nome, pedido(&h, q, corpo), false).await
}

async fn receber_get(
    State(m): State<Mapa>,
    Caminho(nome): Caminho<String>,
    RawQuery(q): RawQuery,
    h: HeaderMap,
) -> Response {
    despachar(m, nome, pedido(&h, q, Bytes::new()), true).await
}

/// Assinatura `prefixo + hex(HMAC-SHA256(segredo, corpo))` num cabecalho, conferida em tempo
/// constante. Meta (`sha256=`), Viber (sem prefixo) e o webhook generico usam esta forma.
pub fn conferir_hmac_hex(
    p: &PedidoWebhook,
    cabecalho: &str,
    prefixo: &str,
    segredo: &str,
) -> Result<(), Recusa> {
    let dado = p
        .cabecalho(cabecalho)
        .and_then(|v| v.strip_prefix(prefixo))
        .ok_or((401, format!("falta {cabecalho}")))?;
    let esperado = super::cripto::hex(&super::cripto::hmac_sha256(segredo.as_bytes(), &p.corpo));
    if super::cripto::iguais(
        dado.trim().to_ascii_lowercase().as_bytes(),
        esperado.as_bytes(),
    ) {
        Ok(())
    } else {
        Err((401, "assinatura nao confere".into()))
    }
}

/// O segredo que a URL carrega (`?chave=`), para o servico que assina com JWT RS256
/// (Teams, Google Chat) -- conferir o JWT pede RSA, que o agente nao tem. Comparado em tempo
/// constante.
pub fn conferir_chave_na_url(p: &PedidoWebhook, segredo: &str) -> Result<(), Recusa> {
    let dada = p.parametro("chave").unwrap_or_default();
    if !dada.is_empty() && super::cripto::iguais(dada.as_bytes(), segredo.as_bytes()) {
        Ok(())
    } else {
        Err((401, "chave da URL nao confere".into()))
    }
}

/// Erro de credencial vira 500 sem detalhe para fora: o motivo vai para o registro de quem
/// opera, nao para quem bateu na porta.
pub fn erro_interno(e: String) -> Recusa {
    (
        500,
        format!("erro interno do canal ({} bytes de detalhe)", e.len()),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp() -> PathBuf {
        std::env::temp_dir().join(format!("phx-caixa-{}", phxclaw_types::new_uuid_v7()))
    }

    fn m(c: &str, id: &str, t: &str) -> (Mensagem, Value) {
        (
            Mensagem {
                conversa: c.into(),
                autor: c.into(),
                id: id.into(),
                texto: Some(t.into()),
            },
            Value::Null,
        )
    }

    #[test]
    fn caixa_sobrevive_ao_reinicio_e_nao_repete_reenvio() {
        let dir = tmp();
        let arq = dir.join("x.caixa.jsonl");
        let c = Caixa::abrir(&arq).unwrap();
        assert_eq!(
            c.anexar(vec![m("a", "1", "oi"), m("a", "1", "oi")])
                .unwrap(),
            1
        );
        assert_eq!(
            c.anexar(vec![m("a", "1", "oi"), m("b", "", "x")]).unwrap(),
            1
        );
        drop(c);
        let c = Caixa::abrir(&arq).unwrap();
        let l = c.ler_desde(None, Duration::ZERO).unwrap();
        assert_eq!(l.len(), 2);
        assert_eq!(l[1].mensagem.as_ref().unwrap().id, "cx2");
        assert_eq!(l[1].cursor.as_deref(), Some("3"));
        let l = c.ler_desde(Some("3"), Duration::from_millis(10)).unwrap();
        assert!(l.is_empty());
        let _ = std::fs::remove_dir_all(dir);
    }
}
