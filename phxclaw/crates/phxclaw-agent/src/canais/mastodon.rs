//! Mastodon (e o resto do fediverso que fala a mesma API): quem menciona o bot fala com o
//! agente, e a resposta volta como mensagem direta (`visibility: direct`) mencionando a
//! pessoa -- no Mastodon a mencao e o enderecamento, sem ela ninguem recebe.
//!
//! Le por `GET /notifications?types[]=mention&since_id=` (o cursor e o id da ultima
//! notificacao). O teto da instancia padrao e 500 caracteres CONTANDO a mencao, entao o
//! pedaco e de 400 e o envio recusa, em vez de cortar, o que passar de 500.

use super::http::{Credencial, Http};
use super::{Entrada, Mensagem, Provedor, Unidade, ordem_numerica, pausa_se_vazio, texto_de_html};
use phxclaw_channel_providers::ProviderEndpointPolicy;
use serde_json::{Value, json};

pub const TETO: usize = 500;

pub struct Mastodon {
    http: Http,
    cred: Credencial,
}

impl Mastodon {
    pub fn novo(
        cred: Credencial,
        base: &str,
        politica: ProviderEndpointPolicy,
    ) -> Result<Self, String> {
        Ok(Self {
            http: Http::novo(base, politica)?,
            cred,
        })
    }
}

/// O texto do post sem as mencoes do comeco (`@bot oi` vira `oi`).
pub fn sem_mencoes(html: &str) -> String {
    let t = texto_de_html(html);
    let mut resto = t.trim_start();
    while resto.starts_with('@') {
        resto = resto
            .split_once(char::is_whitespace)
            .map_or("", |(_, r)| r)
            .trim_start();
    }
    resto.to_string()
}

impl Provedor for Mastodon {
    fn nome(&self) -> &str {
        "mastodon"
    }

    fn limite(&self) -> (usize, Unidade) {
        (400, Unidade::Caractere)
    }

    fn receber(&self, cursor: Option<&str>, espera_seg: u64) -> Result<Vec<Entrada>, String> {
        let v = self.cred.com("receive", |token| {
            let mut q = vec![("types[]", "mention".to_string())];
            match cursor {
                Some(c) => {
                    q.push(("since_id", c.to_string()));
                    q.push(("limit", "40".into()));
                }
                None => q.push(("limit", "1".into())),
            }
            self.http.json(
                self.http
                    .cliente()?
                    .get(self.http.url("/api/v1/notifications"))
                    .bearer_auth(token)
                    .query(&q),
            )
        })?;
        let mut ns: Vec<Value> = v.as_array().cloned().unwrap_or_default();
        ns.sort_by(|a, b| {
            ordem_numerica(
                a["id"].as_str().unwrap_or(""),
                b["id"].as_str().unwrap_or(""),
            )
        });
        if cursor.is_none() {
            let ultimo = ns
                .last()
                .and_then(|n| n["id"].as_str())
                .unwrap_or("0")
                .to_string();
            return Ok(vec![Entrada {
                cursor: Some(ultimo),
                mensagem: None,
            }]);
        }
        let mut lote = Vec::new();
        for n in ns {
            let Some(id) = n["id"].as_str() else { continue };
            let s = &n["status"];
            lote.push(Entrada {
                cursor: Some(id.to_string()),
                mensagem: (n["type"] == "mention" && !s.is_null()).then(|| {
                    let acct = n["account"]["acct"].as_str().unwrap_or("").to_string();
                    Mensagem {
                        conversa: acct.clone(),
                        autor: acct,
                        id: s["id"].as_str().unwrap_or(id).to_string(),
                        texto: s["content"].as_str().map(sem_mencoes),
                    }
                }),
            });
        }
        pausa_se_vazio(&lote, espera_seg);
        Ok(lote)
    }

    fn enviar(&self, conversa: &str, texto: &str) -> Result<String, String> {
        let status = format!("@{conversa} {texto}");
        if status.chars().count() > TETO {
            return Err(format!(
                "post de {} caracteres passa do teto de {TETO} da instancia",
                status.chars().count()
            ));
        }
        self.cred.com("send", |token| {
            let v = self.http.json(
                self.http
                    .cliente()?
                    .post(self.http.url("/api/v1/statuses"))
                    .bearer_auth(token)
                    .json(&json!({"status": status, "visibility": "direct"})),
            )?;
            Ok(v["id"].as_str().unwrap_or("").to_string())
        })
    }
}
