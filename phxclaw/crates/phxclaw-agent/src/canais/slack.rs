//! Slack: le os canais da lista por `conversations.history?oldest=` (o cursor e o `ts` da
//! ultima mensagem vista em cada canal) e responde pelo `SlackProvider` do
//! `phxclaw-channel-providers`.
//!
//! Por que nao o Events API: ele empurra por webhook e exige URL publica com TLS; a leitura
//! por `oldest` funciona atras de NAT, com o mesmo cursor de todo outro provedor. Mensagem
//! com `bot_id` (inclusive a do proprio agente) ou com `subtype` (entrou no canal, mudou o
//! topico) so faz o cursor andar.

use super::http::{Credencial, Http};
use super::{
    Entrada, MapaCursor, Mensagem, Preguicoso, Provedor, Unidade, enviar_pelo, pausa_se_vazio,
};
use phxclaw_channel_providers::{ProviderEndpointPolicy, SlackProvider};
use serde_json::Value;

pub const BASE: &str = "https://slack.com/api";

pub struct Slack {
    http: Http,
    cred: Credencial,
    canais: Vec<String>,
    envio: Preguicoso<SlackProvider>,
}

/// `ts` do Slack ("1700000000.000200") como par de inteiros: comparar como texto erra
/// quando a parte inteira muda de tamanho, e como f64 perde o fim.
fn ts(v: &str) -> (u64, u64) {
    let (a, b) = v.split_once('.').unwrap_or((v, "0"));
    (a.parse().unwrap_or(0), b.parse().unwrap_or(0))
}

impl Slack {
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
                SlackProvider::new_with_origin(
                    broker.clone(),
                    segredo,
                    "slack",
                    base.clone(),
                    politica.clone(),
                )
                .map_err(|e| e.to_string())
            }),
        })
    }

    fn historico(&self, canal: &str, desde: Option<&str>) -> Result<Vec<Value>, String> {
        self.cred.com("receive", |token| {
            let mut q = vec![("channel", canal.to_string())];
            match desde {
                Some(d) => {
                    q.push(("oldest", d.to_string()));
                    q.push(("limit", "100".into()));
                }
                None => q.push(("limit", "1".into())),
            }
            let v = self.http.json(
                self.http
                    .cliente()?
                    .get(self.http.url("/conversations.history"))
                    .bearer_auth(token)
                    .query(&q),
            )?;
            if v["ok"].as_bool() != Some(true) {
                return Err(format!(
                    "slack recusou: {}",
                    v["error"].as_str().unwrap_or("sem detalhe")
                ));
            }
            Ok(v["messages"].as_array().cloned().unwrap_or_default())
        })
    }
}

impl Provedor for Slack {
    fn nome(&self) -> &str {
        "slack"
    }

    /// O Slack aceita ate 40 mil e trunca acima de 4 mil na exibicao; partir em 4 mil e o
    /// que a documentacao dele recomenda.
    fn limite(&self) -> (usize, Unidade) {
        (4000, Unidade::Caractere)
    }

    fn receber(&self, cursor: Option<&str>, espera_seg: u64) -> Result<Vec<Entrada>, String> {
        let mut mapa = MapaCursor::ler(cursor);
        let mut lote = Vec::new();
        for canal in &self.canais {
            let desde = mapa.0.get(canal).cloned();
            let mut msgs = self.historico(canal, desde.as_deref())?;
            msgs.sort_by_key(|m| ts(m["ts"].as_str().unwrap_or("0")));
            if desde.is_none() {
                let ultimo = msgs
                    .last()
                    .and_then(|m| m["ts"].as_str())
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
                let Some(t) = m["ts"].as_str() else { continue };
                mapa.0.insert(canal.clone(), t.to_string());
                let humana = m.get("bot_id").is_none() && m.get("subtype").is_none();
                lote.push(Entrada {
                    cursor: Some(mapa.texto()),
                    mensagem: humana.then(|| Mensagem {
                        conversa: canal.clone(),
                        autor: m["user"].as_str().unwrap_or("").to_string(),
                        id: t.to_string(),
                        texto: m["text"].as_str().map(str::to_string),
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
