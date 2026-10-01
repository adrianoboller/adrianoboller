//! Rocket.Chat pela REST API: le as salas da lista por `{tipo}.history?oldest=` e responde
//! por `chat.postMessage`.
//!
//! `tipo` e `channels` (sala publica), `groups` (privada) ou `im` (conversa direta): o
//! Rocket.Chat separa o historico por tipo de sala e nao deixa um endpoint ler os tres. O
//! cursor e o `ts` (ISO 8601, sempre em UTC e com milissegundos -- da para comparar como
//! texto) da ultima mensagem vista em cada sala. Mensagem do proprio bot e de sistema
//! (`t` preenchido) so faz o cursor andar.

use super::http::{Credencial, Http};
use super::{Entrada, MapaCursor, Mensagem, Provedor, Unidade, pausa_se_vazio};
use phxclaw_channel_providers::ProviderEndpointPolicy;
use reqwest::blocking::RequestBuilder;
use serde_json::{Value, json};

pub struct RocketChat {
    http: Http,
    cred: Credencial,
    usuario: String,
    tipo: String,
    salas: Vec<String>,
}

impl RocketChat {
    /// `usuario` e o `X-User-Id` do bot (nao e segredo: o token e).
    pub fn novo(
        cred: Credencial,
        usuario: String,
        tipo: String,
        salas: Vec<String>,
        base: &str,
        politica: ProviderEndpointPolicy,
    ) -> Result<Self, String> {
        if !matches!(tipo.as_str(), "channels" | "groups" | "im") {
            return Err(format!(
                "tipo de sala invalido: {tipo} (channels, groups ou im)"
            ));
        }
        Ok(Self {
            http: Http::novo(base, politica)?,
            cred,
            usuario,
            tipo,
            salas,
        })
    }

    fn pedir(
        &self,
        uso: &str,
        rb: impl Fn() -> Result<RequestBuilder, String>,
    ) -> Result<Value, String> {
        self.cred.com(uso, |token| {
            let v = self.http.json(
                rb()?
                    .header("X-Auth-Token", token)
                    .header("X-User-Id", &self.usuario),
            )?;
            if v["success"].as_bool() == Some(false) {
                return Err(format!(
                    "rocket.chat recusou: {}",
                    v["error"].as_str().unwrap_or("sem detalhe")
                ));
            }
            Ok(v)
        })
    }
}

impl Provedor for RocketChat {
    fn nome(&self) -> &str {
        "rocketchat"
    }

    /// 5000 e o padrao de `Message_MaxAllowedSize`.
    fn limite(&self) -> (usize, Unidade) {
        (5000, Unidade::Caractere)
    }

    fn receber(&self, cursor: Option<&str>, espera_seg: u64) -> Result<Vec<Entrada>, String> {
        let mut mapa = MapaCursor::ler(cursor);
        let mut lote = Vec::new();
        for sala in &self.salas {
            let desde = mapa.0.get(sala).cloned();
            let url = self.http.url(&format!("/api/v1/{}.history", self.tipo));
            let v = self.pedir("receive", || {
                let mut q = vec![("roomId", sala.clone())];
                match &desde {
                    Some(d) => {
                        q.push(("oldest", d.clone()));
                        q.push(("count", "100".into()));
                    }
                    None => q.push(("count", "1".into())),
                }
                Ok(self.http.cliente()?.get(&url).query(&q))
            })?;
            let mut msgs = v["messages"].as_array().cloned().unwrap_or_default();
            msgs.sort_by(|a, b| a["ts"].as_str().cmp(&b["ts"].as_str()));
            if desde.is_none() {
                let ultimo = msgs
                    .last()
                    .and_then(|m| m["ts"].as_str())
                    .unwrap_or("1970-01-01T00:00:00.000Z")
                    .to_string();
                mapa.0.insert(sala.clone(), ultimo);
                lote.push(Entrada {
                    cursor: Some(mapa.texto()),
                    mensagem: None,
                });
                continue;
            }
            for m in msgs {
                let Some(ts) = m["ts"].as_str() else { continue };
                if desde.as_deref().is_some_and(|d| ts <= d) {
                    continue;
                }
                mapa.0.insert(sala.clone(), ts.to_string());
                let humana = m["u"]["_id"] != self.usuario.as_str() && m.get("t").is_none();
                lote.push(Entrada {
                    cursor: Some(mapa.texto()),
                    mensagem: humana.then(|| Mensagem {
                        conversa: sala.clone(),
                        autor: m["u"]["_id"].as_str().unwrap_or("").to_string(),
                        id: m["_id"].as_str().unwrap_or("").to_string(),
                        texto: m["msg"].as_str().map(str::to_string),
                    }),
                });
            }
        }
        pausa_se_vazio(&lote, espera_seg);
        Ok(lote)
    }

    fn enviar(&self, conversa: &str, texto: &str) -> Result<String, String> {
        let v = self.pedir("send", || {
            Ok(self
                .http
                .cliente()?
                .post(self.http.url("/api/v1/chat.postMessage"))
                .json(&json!({"roomId": conversa, "text": texto})))
        })?;
        Ok(v["message"]["_id"].as_str().unwrap_or("").to_string())
    }
}
