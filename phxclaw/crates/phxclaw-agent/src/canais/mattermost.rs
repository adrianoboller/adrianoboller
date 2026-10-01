//! Mattermost pela API v4: le os canais da lista por `GET /channels/{id}/posts?since=` e
//! responde por `POST /posts`.
//!
//! O `since` do Mattermost devolve o que foi CRIADO OU EDITADO depois do instante; edicao
//! nao e mensagem nova, entao so entra post com `create_at` acima do cursor, e o cursor e o
//! `create_at` (milissegundos) do ultimo post visto em cada canal. Post do proprio bot
//! (`user_id` igual ao dele) e post de sistema (`type` preenchido) so fazem o cursor andar.

use super::http::{Credencial, Http};
use super::{Entrada, MapaCursor, Mensagem, Provedor, Unidade, pausa_se_vazio};
use phxclaw_channel_providers::ProviderEndpointPolicy;
use serde_json::{Value, json};

pub struct Mattermost {
    http: Http,
    cred: Credencial,
    canais: Vec<String>,
    bot: String,
}

impl Mattermost {
    /// `bot` e o `user_id` da conta do bot.
    pub fn novo(
        cred: Credencial,
        bot: String,
        canais: Vec<String>,
        base: &str,
        politica: ProviderEndpointPolicy,
    ) -> Result<Self, String> {
        Ok(Self {
            http: Http::novo(base, politica)?,
            cred,
            canais,
            bot,
        })
    }
}

impl Provedor for Mattermost {
    fn nome(&self) -> &str {
        "mattermost"
    }

    /// 16383 e o teto padrao de post do servidor (`MaxPostSize`).
    fn limite(&self) -> (usize, Unidade) {
        (16_383, Unidade::Caractere)
    }

    fn receber(&self, cursor: Option<&str>, espera_seg: u64) -> Result<Vec<Entrada>, String> {
        let mut mapa = MapaCursor::ler(cursor);
        let mut lote = Vec::new();
        let agora = chrono::Utc::now().timestamp_millis().to_string();
        for canal in &self.canais {
            let Some(desde) = mapa.0.get(canal).cloned() else {
                // Primeira vez: comeca do agora.
                mapa.0.insert(canal.clone(), agora.clone());
                lote.push(Entrada {
                    cursor: Some(mapa.texto()),
                    mensagem: None,
                });
                continue;
            };
            let limiar: i64 = desde.parse().unwrap_or(0);
            let v = self.cred.com("receive", |token| {
                self.http.json(
                    self.http
                        .cliente()?
                        .get(self.http.url(&format!("/api/v4/channels/{canal}/posts")))
                        .bearer_auth(token)
                        .query(&[("since", desde.as_str())]),
                )
            })?;
            let mut posts: Vec<Value> = v["posts"]
                .as_object()
                .map(|m| m.values().cloned().collect())
                .unwrap_or_default();
            posts.retain(|p| p["create_at"].as_i64().unwrap_or(0) > limiar);
            posts.sort_by_key(|p| p["create_at"].as_i64().unwrap_or(0));
            for p in posts {
                mapa.0.insert(
                    canal.clone(),
                    p["create_at"].as_i64().unwrap_or(0).to_string(),
                );
                let humano = p["user_id"] != self.bot.as_str()
                    && p["type"].as_str().unwrap_or("").is_empty();
                lote.push(Entrada {
                    cursor: Some(mapa.texto()),
                    mensagem: humano.then(|| Mensagem {
                        conversa: canal.clone(),
                        autor: p["user_id"].as_str().unwrap_or("").to_string(),
                        id: p["id"].as_str().unwrap_or("").to_string(),
                        texto: p["message"].as_str().map(str::to_string),
                    }),
                });
            }
        }
        pausa_se_vazio(&lote, espera_seg);
        Ok(lote)
    }

    fn enviar(&self, conversa: &str, texto: &str) -> Result<String, String> {
        self.cred.com("send", |token| {
            let v = self.http.json(
                self.http
                    .cliente()?
                    .post(self.http.url("/api/v4/posts"))
                    .bearer_auth(token)
                    .json(&json!({"channel_id": conversa, "message": texto})),
            )?;
            Ok(v["id"].as_str().unwrap_or("").to_string())
        })
    }
}
