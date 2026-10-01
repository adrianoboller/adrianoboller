//! Discord: le os canais da lista pela API REST (`GET /channels/{id}/messages?after=`) e
//! responde pelo `DiscordProvider` do `phxclaw-channel-providers`, que ja mandava
//! `allowed_mentions` vazio -- o agente nunca chama `@everyone` por acidente.
//!
//! Por que REST e nao o gateway de WebSocket: o gateway entrega o evento uma vez e nao
//! deixa perguntar "o que veio desde X"; a REST deixa, e o cursor por canal fica igual ao
//! de todo outro provedor. O preco e o atraso de uma volta (ate 3 s) e o intent
//! MESSAGE_CONTENT ligado no portal do bot, sem o qual o Discord entrega o texto vazio.

use super::http::{Credencial, Http};
use super::{
    Entrada, MapaCursor, Mensagem, Preguicoso, Provedor, Unidade, enviar_pelo, ordem_numerica,
    pausa_se_vazio,
};
use phxclaw_channel_providers::{DiscordProvider, ProviderEndpointPolicy};
use serde_json::Value;

pub const BASE: &str = "https://discord.com/api/v10";

pub struct Discord {
    http: Http,
    cred: Credencial,
    canais: Vec<String>,
    envio: Preguicoso<DiscordProvider>,
}

impl Discord {
    pub fn novo(
        cred: Credencial,
        canais: Vec<String>,
        base: &str,
        politica: ProviderEndpointPolicy,
    ) -> Result<Self, String> {
        let http = Http::novo(base, politica.clone())?;
        let base = base.to_string();
        let (broker, segredo) = (cred.broker(), cred.id());
        Ok(Self {
            http,
            cred,
            canais,
            envio: Preguicoso::novo(move || {
                DiscordProvider::new_with_origin(
                    broker.clone(),
                    segredo,
                    "discord",
                    base.clone(),
                    politica.clone(),
                )
                .map_err(|e| e.to_string())
            }),
        })
    }

    fn mensagens(&self, canal: &str, depois: Option<&str>) -> Result<Vec<Value>, String> {
        self.cred.com("receive", |token| {
            let mut q = vec![("limit", "100".to_string())];
            if let Some(d) = depois {
                q.push(("after", d.to_string()));
            } else {
                q[0].1 = "1".into();
            }
            let v = self.http.json(
                self.http
                    .cliente()?
                    .get(self.http.url(&format!("/channels/{canal}/messages")))
                    .header("Authorization", format!("Bot {token}"))
                    .query(&q),
            )?;
            Ok(v.as_array().cloned().unwrap_or_default())
        })
    }
}

impl Provedor for Discord {
    fn nome(&self) -> &str {
        "discord"
    }

    fn limite(&self) -> (usize, Unidade) {
        (2000, Unidade::Caractere)
    }

    fn receber(&self, cursor: Option<&str>, espera_seg: u64) -> Result<Vec<Entrada>, String> {
        let mut mapa = MapaCursor::ler(cursor);
        let mut lote = Vec::new();
        for canal in &self.canais {
            let depois = mapa.0.get(canal).cloned();
            let mut msgs = self.mensagens(canal, depois.as_deref())?;
            msgs.sort_by(|a, b| {
                ordem_numerica(
                    a["id"].as_str().unwrap_or(""),
                    b["id"].as_str().unwrap_or(""),
                )
            });
            if depois.is_none() {
                // Primeira vez neste canal: comeca do agora, sem responder ao historico.
                let ultimo = msgs
                    .last()
                    .and_then(|m| m["id"].as_str())
                    .unwrap_or("0")
                    .to_string();
                mapa.0.insert(canal.clone(), ultimo);
                lote.push(Entrada {
                    cursor: Some(mapa.texto()),
                    mensagem: None,
                });
                continue;
            }
            for m in msgs {
                let Some(id) = m["id"].as_str() else { continue };
                mapa.0.insert(canal.clone(), id.to_string());
                // Mensagem de bot (inclusive a do proprio agente) so faz o cursor andar:
                // responder a si mesmo seria um laco infinito.
                let de_bot = m["author"]["bot"].as_bool().unwrap_or(false);
                lote.push(Entrada {
                    cursor: Some(mapa.texto()),
                    mensagem: (!de_bot).then(|| Mensagem {
                        conversa: canal.clone(),
                        autor: m["author"]["id"].as_str().unwrap_or("").to_string(),
                        id: id.to_string(),
                        texto: m["content"].as_str().map(str::to_string),
                    }),
                });
            }
        }
        pausa_se_vazio(&lote, espera_seg);
        Ok(lote)
    }

    fn enviar(&self, conversa: &str, texto: &str) -> Result<String, String> {
        enviar_pelo(self.envio.obter()?, conversa, texto)
    }
}
