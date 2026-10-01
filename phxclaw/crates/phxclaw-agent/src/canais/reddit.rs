//! Reddit: mensagens privadas para a conta do bot viram tarefa, e a resposta volta como
//! mensagem privada (`/api/compose`).
//!
//! O inbox do Reddit nao tem cursor: o que se le e "nao lidas", e marcar como lida e o que
//! as tira de la. Entao a ordem e: ler, gravar na `Caixa` em disco, e SO DEPOIS marcar como
//! lidas -- cair no meio relê, e a caixa descarta o repetido pelo id; marcar antes de
//! gravar perderia a mensagem.
//!
//! Login de app "script" (usuario e senha da conta do bot mais o segredo do app, os dois
//! no broker). O token de acesso e pedido a cada volta e nao fica guardado.

use super::caixa::Caixa;
use super::http::{Credencial, Http};
use super::{Entrada, Mensagem, Provedor, Unidade};
use phxclaw_channel_providers::ProviderEndpointPolicy;
use serde_json::Value;
use std::sync::Arc;
use std::time::Duration;

pub const WWW: &str = "https://www.reddit.com";
pub const OAUTH: &str = "https://oauth.reddit.com";

pub struct Reddit {
    caixa: Arc<Caixa>,
    segredo_app: Credencial,
    senha: Credencial,
    app_id: String,
    usuario: String,
    login: Http,
    api: Http,
}

impl Reddit {
    #[allow(clippy::too_many_arguments)]
    pub fn novo(
        caixa: Arc<Caixa>,
        segredo_app: Credencial,
        senha: Credencial,
        app_id: String,
        usuario: String,
        www: &str,
        oauth: &str,
        politica: ProviderEndpointPolicy,
    ) -> Result<Self, String> {
        Ok(Self {
            caixa,
            segredo_app,
            senha,
            app_id,
            usuario,
            login: Http::novo(www, politica.clone())?,
            api: Http::novo(oauth, politica)?,
        })
    }

    /// Login de app "script" e uso do token: dois segredos do broker (o do app e a senha)
    /// e o token derivado, os tres tirados de todo erro pela `Credencial`.
    fn com_token<T>(
        &self,
        uso: &str,
        f: impl FnOnce(&str) -> Result<T, String>,
    ) -> Result<T, String> {
        self.segredo_app.com(uso, |segredo| {
            self.senha.com_derivado(
                uso,
                |senha| {
                    let v = self.login.json(
                        self.login
                            .cliente()?
                            .post(self.login.url("/api/v1/access_token"))
                            .basic_auth(&self.app_id, Some(segredo))
                            .form(&[
                                ("grant_type", "password"),
                                ("username", self.usuario.as_str()),
                                ("password", senha),
                            ]),
                    )?;
                    v["access_token"]
                        .as_str()
                        .map(str::to_string)
                        .ok_or_else(|| format!("login sem access_token: {}", v["error"]))
                },
                f,
            )
        })
    }
}

/// So mensagem privada (`t4`); resposta de comentario e mencao (`t1`) ficam de fora.
pub fn mensagens_do_inbox(v: &Value) -> Vec<Mensagem> {
    v["data"]["children"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|c| c["kind"] == "t4")
        .map(|c| {
            let d = &c["data"];
            let autor = d["author"].as_str().unwrap_or("").to_string();
            Mensagem {
                conversa: autor.clone(),
                autor,
                id: d["name"].as_str().unwrap_or("").to_string(),
                texto: d["body"].as_str().map(str::to_string),
            }
        })
        .collect()
}

impl Provedor for Reddit {
    fn nome(&self) -> &str {
        "reddit"
    }

    fn limite(&self) -> (usize, Unidade) {
        (10_000, Unidade::Caractere)
    }

    fn receber(&self, cursor: Option<&str>, espera_seg: u64) -> Result<Vec<Entrada>, String> {
        self.com_token("receive", |t| {
            let v = self.api.json(
                self.api
                    .cliente()?
                    .get(self.api.url("/message/unread"))
                    .bearer_auth(t)
                    .query(&[("limit", "100")]),
            )?;
            let novas = mensagens_do_inbox(&v);
            if novas.is_empty() {
                return Ok(());
            }
            let nomes = novas
                .iter()
                .map(|m| m.id.clone())
                .collect::<Vec<_>>()
                .join(",");
            self.caixa
                .anexar(novas.into_iter().map(|m| (m, Value::Null)).collect())?;
            // Depois da caixa, nunca antes (ver o topo do modulo).
            self.api.json(
                self.api
                    .cliente()?
                    .post(self.api.url("/api/read_message"))
                    .bearer_auth(t)
                    .form(&[("id", nomes.as_str())]),
            )?;
            Ok(())
        })?;
        let lote = self.caixa.ler_desde(cursor, Duration::ZERO)?;
        // O Reddit conta 100 pedidos por minuto por app; vazio, espera mais que os outros.
        if lote.is_empty() && espera_seg > 0 {
            std::thread::sleep(Duration::from_secs(espera_seg.min(10)));
        }
        Ok(lote)
    }

    fn enviar(&self, conversa: &str, texto: &str) -> Result<String, String> {
        self.com_token("send", |t| {
            let v = self.api.json(
                self.api
                    .cliente()?
                    .post(self.api.url("/api/compose"))
                    .bearer_auth(t)
                    .form(&[
                        ("api_type", "json"),
                        ("to", conversa),
                        ("subject", "Resposta do agente"),
                        ("text", texto),
                    ]),
            )?;
            let erros = &v["json"]["errors"];
            if erros.as_array().is_some_and(|e| !e.is_empty()) {
                return Err(format!("reddit recusou: {erros}"));
            }
            Ok(String::new())
        })
    }
}
