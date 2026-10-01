//! Nostr (NIP-01): quem menciona a chave do bot numa nota (kind 1 com a tag `p`) fala com
//! o agente, e a resposta volta como nota mencionando a pessoa.
//!
//! O que faz a lista de permitidos valer aqui e a assinatura: a conversa e a chave publica
//! de quem escreveu, e qualquer um pode pôr qualquer chave num evento. Todo evento que
//! chega tem o id recalculado e a assinatura Schnorr conferida (`bip340`); o que nao
//! confere e descartado antes de virar mensagem.
//!
//! PARCIAL, e por que: so nota publica. Mensagem direta cifrada (NIP-04 pede AES-CBC,
//! NIP-17/44 pede ChaCha20 e HKDF) nao entra sem crate nova -- e a resposta do agente sai
//! publica, entao so serve para o que pode ser publico. A assinatura nao e de tempo
//! constante (ver `bip340`).
//!
//! O relay guarda os eventos: o cursor e o `created_at` do ultimo tratado e a consulta pede
//! `since` nele. O mesmo segundo volta na consulta seguinte, e o gateway descarta o id
//! repetido -- repetir e o lado certo de errar; perder nao.

use super::bip340;
use super::http::Credencial;
use super::{Entrada, Mensagem, Provedor, Unidade};
use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use std::time::Duration;
use tokio_tungstenite::tungstenite::Message as Ws;

pub struct Nostr {
    relay: String,
    segredo: Credencial,
    /// Chave publica do bot (hex), calculada da secreta na montagem.
    eu: String,
}

/// O id do evento: sha256 da serializacao canonica `[0, pubkey, created_at, kind, tags,
/// content]` (NIP-01). O `serde_json` escapa como a NIP pede (sem espaco, UTF-8 cru).
pub fn id_do_evento(ev: &Value) -> String {
    use sha2::{Digest, Sha256};
    let ser = json!([
        0,
        ev["pubkey"],
        ev["created_at"],
        ev["kind"],
        ev["tags"],
        ev["content"]
    ]);
    bip340::hex(&Sha256::digest(ser.to_string().as_bytes()))
}

/// Id recalculado bate e a assinatura confere pela chave do proprio evento.
pub fn evento_valido(ev: &Value) -> bool {
    let (Some(id), Some(pk), Some(sig)) =
        (ev["id"].as_str(), ev["pubkey"].as_str(), ev["sig"].as_str())
    else {
        return false;
    };
    if id_do_evento(ev) != id {
        return false;
    }
    match (
        bip340::de_hex::<32>(id),
        bip340::de_hex::<32>(pk),
        bip340::de_hex::<64>(sig),
    ) {
        (Ok(m), Ok(p), Ok(s)) => bip340::conferir(&p, &m, &s),
        _ => false,
    }
}

/// Monta e assina um evento kind 1.
pub fn nota(
    segredo: &[u8; 32],
    conteudo: &str,
    tags: Value,
    criado_em: i64,
) -> Result<Value, String> {
    let pk = bip340::hex(&bip340::chave_publica(segredo)?);
    let mut ev = json!({"pubkey": pk, "created_at": criado_em, "kind": 1, "tags": tags,
        "content": conteudo});
    let id = id_do_evento(&ev);
    // aux do BIP-340: nao precisa ser secreto, so diferente a cada assinatura.
    let aux: [u8; 32] = {
        use sha2::{Digest, Sha256};
        let agora = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        Sha256::digest(format!("{agora}{id}").as_bytes()).into()
    };
    let sig = bip340::assinar(segredo, &bip340::de_hex(&id)?, &aux)?;
    ev["id"] = json!(id);
    ev["sig"] = json!(bip340::hex(&sig));
    Ok(ev)
}

/// `wss://` sempre; `ws://` so em loopback, pela mesma regra do HTTP sem TLS.
pub fn conferir_relay(url: &str) -> Result<(), String> {
    let u = reqwest::Url::parse(url).map_err(|e| format!("{url}: {e}"))?;
    let loopback = matches!(
        u.host_str(),
        Some("127.0.0.1" | "localhost" | "[::1]" | "::1")
    );
    match u.scheme() {
        "wss" => Ok(()),
        "ws" if loopback => Ok(()),
        _ => Err(format!("{url}: relay sem TLS so em loopback")),
    }
}

impl Nostr {
    pub fn novo(relay: String, segredo: Credencial) -> Result<Self, String> {
        conferir_relay(&relay)?;
        let eu = segredo.com("probe", |s| {
            Ok(bip340::hex(&bip340::chave_publica(&bip340::de_hex(s)?)?))
        })?;
        Ok(Self { relay, segredo, eu })
    }

    pub fn chave_publica(&self) -> &str {
        &self.eu
    }

    fn bloquear<T>(
        &self,
        f: impl std::future::Future<Output = Result<T, String>>,
    ) -> Result<T, String> {
        tokio::runtime::Handle::current().block_on(f)
    }
}

impl Provedor for Nostr {
    fn nome(&self) -> &str {
        "nostr"
    }

    fn limite(&self) -> (usize, Unidade) {
        (2000, Unidade::Caractere)
    }

    fn receber(&self, cursor: Option<&str>, espera_seg: u64) -> Result<Vec<Entrada>, String> {
        let agora = chrono::Utc::now().timestamp();
        let Some(desde) = cursor.and_then(|c| c.parse::<i64>().ok()) else {
            // Primeira vez: comeca do agora.
            return Ok(vec![Entrada {
                cursor: Some(agora.to_string()),
                mensagem: None,
            }]);
        };
        let eu = self.eu.clone();
        let eventos = self.bloquear(async {
            let (mut ws, _) = tokio_tungstenite::connect_async(self.relay.as_str())
                .await
                .map_err(|e| format!("relay: {e}"))?;
            let sub = "phxclaw";
            let req = json!(["REQ", sub, {"kinds": [1], "#p": [eu], "since": desde, "limit": 200}]);
            ws.send(Ws::text(req.to_string()))
                .await
                .map_err(|e| format!("relay: {e}"))?;
            let mut eventos = Vec::new();
            let mut eose = false;
            let fim = tokio::time::Instant::now() + Duration::from_secs(espera_seg.max(1));
            loop {
                // Depois do EOSE, o que ja veio basta; sem nada, espera o ao vivo ate o prazo.
                if eose && !eventos.is_empty() {
                    break;
                }
                let Ok(m) = tokio::time::timeout_at(fim, ws.next()).await else {
                    break;
                };
                let t = match m {
                    None => break,
                    Some(Err(e)) => return Err(format!("relay: {e}")),
                    Some(Ok(Ws::Text(t))) => t,
                    Some(Ok(_)) => continue,
                };
                let v: Value = serde_json::from_str(t.as_str()).unwrap_or(Value::Null);
                match v[0].as_str() {
                    Some("EVENT") if v[1] == sub => eventos.push(v[2].clone()),
                    Some("EOSE") => eose = true,
                    Some("CLOSED") => break,
                    _ => {}
                }
            }
            let _ = ws.send(Ws::text(json!(["CLOSE", sub]).to_string())).await;
            let _ = ws.close(None).await;
            Ok(eventos)
        })?;
        let mut eventos: Vec<Value> = eventos.into_iter().filter(evento_valido).collect();
        eventos.sort_by_key(|e| e["created_at"].as_i64().unwrap_or(0));
        let mut lote: Vec<Entrada> = eventos
            .into_iter()
            .map(|e| {
                let de = e["pubkey"].as_str().unwrap_or("").to_string();
                Entrada {
                    cursor: Some(e["created_at"].as_i64().unwrap_or(desde).to_string()),
                    mensagem: (de != self.eu).then(|| Mensagem {
                        conversa: de.clone(),
                        autor: de,
                        id: e["id"].as_str().unwrap_or("").to_string(),
                        texto: e["content"].as_str().map(str::to_string),
                    }),
                }
            })
            .collect();
        if lote.is_empty() {
            lote.push(Entrada {
                cursor: Some(desde.to_string()),
                mensagem: None,
            });
        }
        Ok(lote)
    }

    fn enviar(&self, conversa: &str, texto: &str) -> Result<String, String> {
        bip340::de_hex::<32>(conversa).map_err(|e| format!("conversa nao e chave: {e}"))?;
        let ev = self.segredo.com("send", |s| {
            nota(
                &bip340::de_hex(s)?,
                texto,
                json!([["p", conversa]]),
                chrono::Utc::now().timestamp(),
            )
        })?;
        let id = ev["id"].as_str().unwrap_or("").to_string();
        self.bloquear(async {
            let (mut ws, _) = tokio_tungstenite::connect_async(self.relay.as_str())
                .await
                .map_err(|e| format!("relay: {e}"))?;
            ws.send(Ws::text(json!(["EVENT", ev]).to_string()))
                .await
                .map_err(|e| format!("relay: {e}"))?;
            let fim = tokio::time::Instant::now() + Duration::from_secs(10);
            let r = loop {
                let Ok(Some(Ok(m))) = tokio::time::timeout_at(fim, ws.next()).await else {
                    break Err("relay nao confirmou o evento (OK)".to_string());
                };
                let Ws::Text(t) = m else { continue };
                let v: Value = serde_json::from_str(t.as_str()).unwrap_or(Value::Null);
                if v[0] == "OK" && v[1] == id.as_str() {
                    break if v[2] == true {
                        Ok(id.clone())
                    } else {
                        Err(format!("relay recusou: {}", v[3]))
                    };
                }
            };
            let _ = ws.close(None).await;
            r
        })
    }
}
