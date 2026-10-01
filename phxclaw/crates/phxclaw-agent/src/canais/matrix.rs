//! Matrix pela API cliente-servidor: `GET /sync?since=` com espera longa e o `next_batch`
//! como cursor, que e exatamente o "o que veio desde X" que o laco pede.
//!
//! A primeira volta sem cursor so pega o `next_batch` (sync com `timeout=0` e filtro de
//! uma linha por sala): sem isso o agente responderia ao historico inteiro das salas. As
//! mensagens do proprio usuario do bot so fazem o cursor andar. Salas cifradas (E2EE) nao
//! entram: o texto chega como `m.room.encrypted`, e decifrar pede Olm/Megolm.

use super::http::{Credencial, Http, trecho};
use super::{Entrada, Mensagem, Provedor, Unidade};
use phxclaw_channel_providers::ProviderEndpointPolicy;
use serde_json::json;

pub struct Matrix {
    http: Http,
    cred: Credencial,
    usuario: String,
}

impl Matrix {
    /// `usuario` e o id do bot (`@agente:servidor`), para ignorar o que ele mesmo escreveu.
    pub fn novo(
        cred: Credencial,
        usuario: String,
        base: &str,
        politica: ProviderEndpointPolicy,
    ) -> Result<Self, String> {
        Ok(Self {
            http: Http::novo(base, politica)?,
            cred,
            usuario,
        })
    }
}

impl Provedor for Matrix {
    fn nome(&self) -> &str {
        "matrix"
    }

    /// O evento inteiro cabe em 64 KiB; 30 mil bytes de corpo deixam folga ao envelope.
    fn limite(&self) -> (usize, Unidade) {
        (30_000, Unidade::Byte)
    }

    fn receber(&self, cursor: Option<&str>, espera_seg: u64) -> Result<Vec<Entrada>, String> {
        let v = self.cred.com("receive", |token| {
            let mut q = vec![("timeout", (espera_seg * 1000).to_string())];
            match cursor {
                Some(c) => q.push(("since", c.to_string())),
                None => {
                    q[0].1 = "0".into();
                    q.push((
                        "filter",
                        json!({"room": {"timeline": {"limit": 1}}}).to_string(),
                    ));
                }
            }
            self.http.json(
                self.http
                    .cliente()?
                    .get(self.http.url("/_matrix/client/v3/sync"))
                    .bearer_auth(token)
                    .query(&q),
            )
        })?;
        let proximo = v["next_batch"]
            .as_str()
            .ok_or("sync sem next_batch")?
            .to_string();
        let mut lote = Vec::new();
        if cursor.is_some()
            && let Some(salas) = v["rooms"]["join"].as_object()
        {
            for (sala, dados) in salas {
                for e in dados["timeline"]["events"].as_array().into_iter().flatten() {
                    if e["type"] != "m.room.message" || e["sender"] == self.usuario.as_str() {
                        continue;
                    }
                    let c = &e["content"];
                    lote.push(Entrada {
                        cursor: None,
                        mensagem: Some(Mensagem {
                            conversa: sala.clone(),
                            autor: e["sender"].as_str().unwrap_or("").to_string(),
                            id: e["event_id"].as_str().unwrap_or("").to_string(),
                            texto: (c["msgtype"] == "m.text")
                                .then(|| c["body"].as_str().map(str::to_string))
                                .flatten(),
                        }),
                    });
                }
            }
        }
        // O `next_batch` so vale depois do lote inteiro: ele confirma todas as salas de uma
        // vez, entao vai no ultimo item (ou num item vazio, se nao veio nada).
        match lote.last_mut() {
            Some(u) => u.cursor = Some(proximo),
            None => lote.push(Entrada {
                cursor: Some(proximo),
                mensagem: None,
            }),
        }
        Ok(lote)
    }

    fn enviar(&self, conversa: &str, texto: &str) -> Result<String, String> {
        // O id da transacao torna o reenvio idempotente: o servidor nao duplica.
        let txn = phxclaw_types::new_uuid_v7().to_string();
        self.cred.com("send", |token| {
            let v = self.http.json(
                self.http
                    .cliente()?
                    .put(self.http.url(&format!(
                        "/_matrix/client/v3/rooms/{}/send/m.room.message/{txn}",
                        trecho(conversa)
                    )))
                    .bearer_auth(token)
                    .json(&json!({"msgtype": "m.text", "body": texto})),
            )?;
            Ok(v["event_id"].as_str().unwrap_or("").to_string())
        })
    }
}
