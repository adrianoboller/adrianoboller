//! HashiCorp Vault, motor KV versao 2: `GET /v1/<montagem>/data/<caminho>?version=N` com o
//! `X-Vault-Token`. O token e o guardado (`phxclaw cofre vault-token`) ou o `client_token` do
//! login AppRole (`POST /v1/auth/<montagem>/login` com `role_id` e o `secret_id` guardado),
//! que fica so em memoria ate o `lease_duration`.
//!
//! O KV v2 guarda um MAPA por caminho: o `campo` da referencia escolhe a chave. Sem `campo`,
//! vale so quando o mapa tem uma chave -- adivinhar qual de varias seria entregar a senha do
//! banco a quem pediu o usuario.

use super::{
    CfgVault, ConfigCofres, MetodoVault, Saida, TokenEmMemoria, VAULT_SECRET_ID, VAULT_TOKEN, base,
    campo_do_objeto, json_de, limpar, motivo_do_status, segmento,
};
use phxclaw_http_client::HttpRequestSpec;
use phxclaw_secret_broker::SecretValue;
use phxclaw_secret_broker::cofre::{BoxFut, CofreExterno, ReferenciaExterna};
use serde_json::json;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Sem `lease_duration` na resposta do login, o token vale isto do nosso lado.
const LEASE_PADRAO: u64 = 300;

pub struct Vault {
    raiz: PathBuf,
    url: String,
    montagem: String,
    approle: String,
    cfg: CfgVault,
    saida: Saida,
    token: TokenEmMemoria,
}

fn caminho_codificado(c: &str, o_que: &str) -> Result<String, String> {
    c.trim_matches('/')
        .split('/')
        .map(|s| segmento(s, o_que))
        .collect::<Result<Vec<_>, _>>()
        .map(|v| v.join("/"))
}

impl Vault {
    pub fn novo(raiz: &Path, v: &CfgVault, cfg: &ConfigCofres) -> Result<Self, String> {
        let saida = Saida::nova(&v.url, cfg)?;
        if v.metodo == MetodoVault::Approle && v.role_id.as_deref().is_none_or(str::is_empty) {
            return Err("Vault com metodo approle pede cofres.vault.role_id".into());
        }
        Ok(Self {
            raiz: raiz.to_path_buf(),
            url: v.url.trim().trim_end_matches('/').to_string(),
            montagem: caminho_codificado(&v.montagem, "cofres.vault.montagem")?,
            approle: caminho_codificado(&v.approle_montagem, "cofres.vault.approle_montagem")?,
            cfg: v.clone(),
            saida,
            token: TokenEmMemoria::default(),
        })
    }

    fn pedido(&self, url: String) -> HttpRequestSpec {
        let mut spec = HttpRequestSpec::get(url);
        if let Some(ns) = self.cfg.namespace.as_deref().filter(|n| !n.is_empty()) {
            spec.headers
                .insert("X-Vault-Namespace".into(), ns.to_string());
        }
        spec
    }

    async fn token(&self) -> Result<SecretValue, String> {
        if self.cfg.metodo == MetodoVault::Token {
            return base(&self.raiz, &VAULT_TOKEN);
        }
        if let Some(t) = self.token.valido() {
            return Ok(t);
        }
        let secret_id = base(&self.raiz, &VAULT_SECRET_ID)?;
        let mut spec = self.pedido(format!("{}/v1/auth/{}/login", self.url, self.approle));
        spec.method = "POST".into();
        spec.json = Some(json!({
            "role_id": self.cfg.role_id.clone().unwrap_or_default(),
            "secret_id": secret_id.expose(),
        }));
        let r = self.saida.enviar(spec).await;
        let r = r.map_err(|e| limpar(format!("login AppRole: {e}"), &[&secret_id]))?;
        if r.status != 200 {
            return Err(format!(
                "login AppRole: {}",
                motivo_do_status(r.status, None)
            ));
        }
        let v = json_de(&r);
        let token = v["auth"]["client_token"]
            .as_str()
            .filter(|t| !t.is_empty())
            .ok_or("login AppRole sem auth.client_token")?;
        let token = SecretValue::new(token.to_string());
        let lease = v["auth"]["lease_duration"].as_u64().unwrap_or(LEASE_PADRAO);
        self.token
            .guardar(token.clone(), Duration::from_secs(lease.max(1)));
        Ok(token)
    }

    async fn ler_valor(&self, r: &ReferenciaExterna) -> Result<SecretValue, String> {
        let caminho = caminho_codificado(&r.caminho, "caminho")?;
        let query = match r.versao.as_deref() {
            None => String::new(),
            Some(v) => {
                let n: u64 = v
                    .parse()
                    .map_err(|_| format!("versao do Vault e numero inteiro, nao {v:?}"))?;
                format!("?version={n}")
            }
        };
        let url = format!("{}/v1/{}/data/{caminho}{query}", self.url, self.montagem);
        for tentativa in 0..2 {
            let token = self.token().await?;
            let mut spec = self.pedido(url.clone());
            spec.headers
                .insert("X-Vault-Token".into(), token.expose().to_string());
            let resp = self.saida.enviar(spec).await;
            let resp = resp.map_err(|e| limpar(e, &[&token]))?;
            // AppRole: o token em memoria pode ter sido revogado antes do prazo; um login
            // novo, uma vez.
            if resp.status == 403 && self.cfg.metodo == MetodoVault::Approle && tentativa == 0 {
                self.token.esquecer();
                continue;
            }
            let v = json_de(&resp);
            if resp.status != 200 {
                let apagada = resp.status == 404
                    && v["data"]["metadata"]["deletion_time"]
                        .as_str()
                        .is_some_and(|t| !t.is_empty());
                return Err(if apagada {
                    "a versao pedida foi apagada no Vault (404)".into()
                } else {
                    motivo_do_status(resp.status, None)
                });
            }
            let dados = &v["data"]["data"];
            if dados.is_null() {
                return Err("a versao pedida foi destruida no Vault".into());
            }
            return match r.campo.as_deref() {
                Some(c) => campo_do_objeto(dados, c),
                None => match dados.as_object() {
                    Some(o) if o.len() == 1 => {
                        let (k, _) = o.iter().next().expect("uma chave");
                        campo_do_objeto(dados, k)
                    }
                    Some(o) => Err(format!(
                        "o caminho guarda {} campos: diga o 'campo' (campos: {})",
                        o.len(),
                        o.keys().cloned().collect::<Vec<_>>().join(", ")
                    )),
                    None => Err("o Vault devolveu dados fora da forma do KV v2".into()),
                },
            };
        }
        Err("o Vault recusou o token de novo depois de um login novo (403)".into())
    }
}

impl CofreExterno for Vault {
    fn tipo(&self) -> &'static str {
        "vault"
    }

    fn ler<'a>(&'a self, r: &'a ReferenciaExterna) -> BoxFut<'a, Result<SecretValue, String>> {
        Box::pin(self.ler_valor(r))
    }
}
