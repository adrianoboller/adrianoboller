//! O que todo provedor HTTP repete: ler o segredo por concessao curta e falar JSON com o
//! servico sem deixar o segredo escapar no erro.
//!
//! Por que o scrub mora em `Credencial::com` e nao em cada chamada: o erro do reqwest traz
//! a URL (no Telegram a URL carrega o token) e o corpo de erro de alguns servicos ecoa o
//! pedido. Tirar o segredo na saida da concessao cobre todo caminho de erro, inclusive o que
//! alguem escrever amanha sem lembrar do scrub.

use phxclaw_channel_providers::ProviderEndpointPolicy;
use phxclaw_secret_broker::{SecretBroker, SecretValue, scrub_text};
use reqwest::blocking::{Client, RequestBuilder};
use serde_json::Value;
use std::sync::{Arc, OnceLock};
use std::time::Duration;
use uuid::Uuid;

/// Um segredo do canal no broker. Nao guarda o valor: cada uso pede uma concessao de
/// 30 s, usa, e devolve.
#[derive(Clone)]
pub struct Credencial {
    broker: Arc<SecretBroker>,
    id: Uuid,
    canal: String,
}

impl Credencial {
    pub fn nova(broker: Arc<SecretBroker>, id: Uuid, canal: impl Into<String>) -> Self {
        Self {
            broker,
            id,
            canal: canal.into(),
        }
    }

    /// O broker e o id, para os provedores do `phxclaw-channel-providers`, que fazem a
    /// propria concessao.
    pub fn broker(&self) -> Arc<SecretBroker> {
        self.broker.clone()
    }

    pub fn id(&self) -> Uuid {
        self.id
    }

    /// Roda `f` com o segredo. `uso` e send, receive, probe ou verify (ver
    /// `canais::escopos`). Todo erro que sai daqui passou pelo scrub do segredo.
    pub fn com<T>(
        &self,
        uso: &str,
        f: impl FnOnce(&str) -> Result<T, String>,
    ) -> Result<T, String> {
        let escopo = format!("channel:{}:{uso}", self.canal);
        let concessao = self
            .broker
            .issue_lease(
                self.id,
                format!("channel.provider.{}", self.canal),
                &escopo,
                30,
            )
            .map_err(|e| e.to_string())?;
        let valor = self
            .broker
            .resolve(concessao.uuid, &escopo)
            .map_err(|e| e.to_string());
        let r = valor.and_then(|v| limpar(v.expose(), f(v.expose())));
        let _ = self.broker.revoke_lease(concessao.uuid);
        r
    }

    /// Segredo -> token derivado (o login de client credentials do Teams, o
    /// `tenant_access_token` do Feishu) -> uso. Token derivado tambem e segredo: sai de todo
    /// erro, pela mesma `limpar`, e nao sobrevive a chamada -- nada o guarda fora do broker.
    pub fn com_derivado<T>(
        &self,
        uso: &str,
        obter: impl FnOnce(&str) -> Result<String, String>,
        usar: impl FnOnce(&str) -> Result<T, String>,
    ) -> Result<T, String> {
        self.com(uso, |s| {
            let t = obter(s)?;
            limpar(&t, usar(&t))
        })
    }
}

/// Tira `segredo` do erro. Uma funcao so para o segredo do broker e para o token que
/// nasce dele, para as duas limpezas nao divergirem.
pub fn limpar<T>(segredo: &str, r: Result<T, String>) -> Result<T, String> {
    r.map_err(|e| scrub_text(&e, &partes(segredo)))
}

/// O segredo e, quando ele e uma URL (o webhook de saida do Google Chat), cada valor da
/// consulta dela: o servidor que ecoa o pedido devolve `token=...` solto, sem o resto da
/// URL, e tirar so a URL inteira deixava o pedaco passar (achado pelo teste de eco).
fn partes(segredo: &str) -> Vec<SecretValue> {
    let mut v = vec![SecretValue::new(segredo.to_string())];
    if let Ok(u) = reqwest::Url::parse(segredo) {
        v.extend(
            u.query_pairs()
                .map(|(_, x)| x.into_owned())
                .filter(|x| x.len() >= 8)
                .map(SecretValue::new),
        );
    }
    v
}

/// Cliente de um servico: a origem passa pela politica de destinos na criacao e a cada
/// pedido, e o cliente bloqueante nasce na primeira chamada -- o laco chama de dentro de
/// `spawn_blocking`, e criar o cliente bloqueante dentro do runtime assincrono derruba o
/// processo.
pub struct Http {
    base: String,
    politica: ProviderEndpointPolicy,
    cliente: OnceLock<Client>,
}

impl Http {
    pub fn novo(base: impl Into<String>, politica: ProviderEndpointPolicy) -> Result<Self, String> {
        let base = base.into().trim_end_matches('/').to_string();
        politica.validate(&base).map_err(|e| e.to_string())?;
        Ok(Self {
            base,
            politica,
            cliente: OnceLock::new(),
        })
    }

    pub fn base(&self) -> &str {
        &self.base
    }

    pub fn url(&self, caminho: &str) -> String {
        format!("{}{caminho}", self.base)
    }

    /// Confere outra origem pela mesma politica (o `serviceUrl` que o Teams manda no
    /// webhook e o que se responde: sem esta conferencia ele seria um SSRF de graca).
    pub fn conferir(&self, url: &str) -> Result<(), String> {
        self.politica
            .validate(url)
            .map(|_| ())
            .map_err(|e| e.to_string())
    }

    pub fn cliente(&self) -> Result<&Client, String> {
        self.politica
            .validate(&self.base)
            .map_err(|e| e.to_string())?;
        if let Some(c) = self.cliente.get() {
            return Ok(c);
        }
        let c = Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(40))
            .user_agent("PhxClaw canais")
            // Redirecionamento desligado: o reqwest tira o `Authorization` ao trocar de
            // origem, mas nao cabecalho proprio (`X-Auth-Token` do Rocket.Chat,
            // `X-Viber-Auth-Token`), que seguiria para o host do 3xx. E 3xx e erro em
            // `json` (fora de 2xx), nunca corpo vazio aceito como sucesso.
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|e| e.without_url().to_string())?;
        Ok(self.cliente.get_or_init(|| c))
    }

    /// Manda o pedido e devolve o corpo em JSON (corpo vazio vira `null`). Status fora de
    /// 2xx e erro com o codigo e o comeco do corpo; a URL nunca entra no erro.
    pub fn json(&self, pedido: RequestBuilder) -> Result<Value, String> {
        let (status, corpo) = self.texto(pedido)?;
        if !(200..300).contains(&status) {
            return Err(format!("HTTP {status}: {}", cortar(&corpo, 300)));
        }
        if corpo.trim().is_empty() {
            return Ok(Value::Null);
        }
        serde_json::from_str(&corpo).map_err(|e| format!("resposta nao e JSON: {e}"))
    }

    /// Status e corpo, sem julgar o status.
    pub fn texto(&self, pedido: RequestBuilder) -> Result<(u16, String), String> {
        let r = pedido.send().map_err(|e| e.without_url().to_string())?;
        let status = r.status().as_u16();
        let corpo = r.text().map_err(|e| e.without_url().to_string())?;
        Ok((status, corpo))
    }
}

/// Corta no limite de caracteres, sem partir caractere.
pub fn cortar(s: &str, max: usize) -> String {
    s.chars().take(max).collect()
}

/// Politica para um servico que o operador hospeda (Matrix, Mattermost, signal-cli...): a
/// origem configurada entra na lista, e HTTP sem TLS so vale para loopback -- token em
/// texto claro na rede seria vazamento.
pub fn politica_para(base: &str) -> Result<ProviderEndpointPolicy, String> {
    let url = reqwest::Url::parse(base).map_err(|e| format!("{base}: {e}"))?;
    let origem = url.origin().ascii_serialization();
    let loopback = matches!(
        url.host_str(),
        Some("127.0.0.1" | "localhost" | "[::1]" | "::1")
    );
    if url.scheme() == "http" && !loopback {
        return Err(format!("{origem}: HTTP sem TLS so em loopback"));
    }
    Ok(ProviderEndpointPolicy::locked_defaults()
        .with_origin(origem)
        .allow_http(loopback))
}

/// Codifica um pedaco de caminho (id de sala do Matrix tem `!` e `:`).
pub fn trecho(s: &str) -> String {
    let mut u = reqwest::Url::parse("http://x/").expect("url fixa");
    u.path_segments_mut().expect("base").push(s);
    u.path().trim_start_matches('/').to_string()
}
