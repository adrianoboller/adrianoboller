//! `phxclaw canal <nome>`: monta o provedor do ambiente e devolve o `Canal` (e as rotas
//! HTTP, para quem recebe por webhook). Um so ponto de entrada para todos os canais, para
//! a regra de onde mora o segredo e de quem pode falar nao divergir de canal para canal.
//!
//! Convencao: `PHXCLAW_<NOME>_<CHAVE>`. Segredo lido do ambiente vai direto ao broker da
//! pasta e o processo nao o guarda; sem a variavel, vale o envelope guardado antes -- o
//! segredo nao precisa ficar no ambiente depois da primeira vez. `PHXCLAW_<NOME>_PERMITIDOS`
//! e a lista de conversas, obrigatoria: lista vazia seria um canal que ninguem usa ou, pior,
//! um que alguem "conserta" liberando todo mundo.

use super::caixa::{Caixa, Recebedor};
use super::http::{Credencial, politica_para};
use super::{Canal, Provedor, Registro, broker_em, guardar_do_canal, lista, segredo_guardado};
use phxclaw_channel_providers::ProviderEndpointPolicy;
use phxclaw_secret_broker::{SecretBroker, SecretValue};
use std::collections::BTreeSet;
use std::path::Path;
use std::sync::Arc;

/// Os canais que este agente sabe ligar, na ordem da ajuda.
pub const CANAIS: &[&str] = &[
    "telegram",
    "discord",
    "slack",
    "whatsapp",
    "teams",
    "matrix",
    "email",
    "webhook",
    "webchat",
    "signal",
    "googlechat",
    "sms",
    "mattermost",
    "rocketchat",
    "zulip",
    "irc",
    "xmpp",
    "mastodon",
    "line",
    "viber",
    "messenger",
    "feishu",
    "reddit",
    "twitch",
    "nostr",
];

pub struct Ligado {
    pub canal: Canal,
    /// Rotas de entrada (webhook, webchat). None: o canal pergunta ao servico.
    pub rotas: Option<axum::Router>,
}

pub type Var<'a> = &'a dyn Fn(&str) -> Option<String>;

struct Ctx<'a> {
    nome: &'a str,
    prefixo: String,
    pasta: &'a Path,
    broker: Arc<SecretBroker>,
    var: Var<'a>,
}

/// Valor de chave booleana do catalogo (`K::B`, e o `TLS`): o `true`/`false` do config.json
/// chega como texto. So os sins e os naos conhecidos decidem; o resto fica no `padrao`, que e
/// o lado seguro de quem chama. Recebe o valor lido, e nao o nome da chave, para a leitura
/// continuar sendo um `ctx.cfg("…")` que o levantamento do fonte enxerga.
fn sim_ou_nao(valor: Option<String>, padrao: bool) -> bool {
    match valor.map(|v| v.trim().to_ascii_lowercase()).as_deref() {
        Some("sim" | "true" | "1" | "yes" | "on") => true,
        Some("nao" | "false" | "0" | "no" | "off") => false,
        _ => padrao,
    }
}

impl Ctx<'_> {
    fn chave(&self, k: &str) -> String {
        format!("PHXCLAW_{}_{k}", self.prefixo)
    }

    fn cfg(&self, k: &str) -> Option<String> {
        (self.var)(&self.chave(k)).filter(|v| !v.trim().is_empty())
    }

    /// TLS do canal de soquete: ligado por padrao (`<CANAL>_TLS=false` desliga, e ai so
    /// loopback); `<CANAL>_CA` troca as raizes publicas por SO a autoridade do PEM.
    fn tls(&self) -> Result<Option<super::tls::Tls>, String> {
        // O catalogo declara `TLS` booleano: o `false` do config.json chega aqui como texto,
        // e so «nao» deixaria o `false` ligar o TLS calado.
        if !sim_ou_nao(self.cfg("TLS"), true) {
            return Ok(None);
        }
        super::tls::Tls::da_config(self.cfg("CA").as_deref()).map(Some)
    }

    fn exigir(&self, k: &str) -> Result<String, String> {
        self.cfg(k)
            .ok_or_else(|| format!("falta {}", self.chave(k)))
    }

    fn segredo_opcional(&self, k: &str) -> Result<Option<Credencial>, String> {
        self.segredo_de(k, self.cfg(k))
    }

    /// `valor` vai para o broker com o nome `<canal>-<k>`; sem valor, vale o guardado.
    fn segredo_de(&self, k: &str, valor: Option<String>) -> Result<Option<Credencial>, String> {
        let nome = format!("{}-{}", self.nome, k.to_ascii_lowercase());
        let id = match valor.filter(|v| !v.is_empty()) {
            Some(v) => Some(guardar_do_canal(
                &self.broker,
                &nome,
                self.nome,
                SecretValue::new(v),
            )?),
            None => segredo_guardado(&self.broker, &nome, "canais")?.map(|d| d.uuid),
        };
        Ok(id.map(|id| Credencial::nova(self.broker.clone(), id, self.nome)))
    }

    fn segredo(&self, k: &str) -> Result<Credencial, String> {
        self.segredo_opcional(k)?
            .ok_or_else(|| format!("falta {} (nem no ambiente, nem guardado)", self.chave(k)))
    }

    fn caixa(&self, sufixo: &str) -> Result<Arc<Caixa>, String> {
        Caixa::abrir(
            self.pasta
                .join(format!("{}{sufixo}.caixa.jsonl", self.nome)),
        )
    }

    /// A base do servico: `PHXCLAW_<NOME>_BASE`, ou a oficial. A politica aceita so ela.
    fn base(&self, padrao: Option<&str>) -> Result<(String, ProviderEndpointPolicy), String> {
        let base = match (self.cfg("BASE"), padrao) {
            (Some(b), _) => b,
            (None, Some(p)) => p.to_string(),
            (None, None) => return Err(format!("falta {}", self.chave("BASE"))),
        };
        let p = politica_para(&base)?;
        Ok((base, p))
    }
}

fn recebedor<T: Recebedor>(x: &Arc<T>) -> Option<axum::Router> {
    let r: Arc<dyn Recebedor> = x.clone();
    Some(super::caixa::rotas(vec![r]))
}

/// Liga o canal `nome` com a configuracao de `var` (o ambiente, no binario).
pub async fn ligar(
    nome: &str,
    pasta: &Path,
    var: Var<'_>,
    log: Registro,
) -> Result<Ligado, String> {
    if nome == "telegram" {
        // Os nomes saem do catalogo; o `var` injetado (a CLI passa `config::por_variavel`)
        // continua sendo o leitor, por nome de variavel como nos outros canais.
        let var_token = crate::config::variavel("canais.telegram.bot_token");
        let token = var(var_token)
            .filter(|t| !t.trim().is_empty())
            .ok_or_else(|| format!("falta {var_token}"))?;
        let chats = crate::canal::chats_da_lista(
            &var(crate::config::variavel("canais.telegram.chats")).unwrap_or_default(),
        )?;
        let canal =
            crate::canal::ligar_telegram(pasta, SecretValue::new(token), chats, log).await?;
        return Ok(Ligado { canal, rotas: None });
    }
    if !CANAIS.contains(&nome) {
        return Err(format!(
            "canal desconhecido: {nome} (conhecidos: {})",
            CANAIS.join(", ")
        ));
    }
    let prefixo = match nome {
        // PHXCLAW_EMAIL_* ja e da ferramenta send_email (remetente e destinos permitidos).
        "email" => "EMAIL_CANAL".to_string(),
        n => n.to_ascii_uppercase(),
    };
    let ctx = Ctx {
        nome,
        prefixo,
        pasta,
        broker: broker_em(pasta)?,
        var,
    };
    let permitidos = lista(&ctx.cfg("PERMITIDOS").unwrap_or_default());
    if permitidos.is_empty() {
        return Err(format!(
            "{} vazia: ninguem poderia falar com o agente",
            ctx.chave("PERMITIDOS")
        ));
    }
    // XMPP: parte local e dominio do JID nao distinguem caixa (RFC 7622), e o provedor
    // entrega a conversa e o autor em minusculas; a lista do portao tem de estar na mesma
    // forma, senao `Ana@X.org` escrito pelo operador nunca bate com quem fala.
    let permitidos: BTreeSet<String> = if nome == "xmpp" {
        permitidos
            .iter()
            .map(|p| super::xmpp::jid_normal(p))
            .collect()
    } else {
        permitidos
    };
    let conversas: Vec<String> = permitidos.iter().cloned().collect();
    let (provedor, rotas): (Arc<dyn Provedor>, Option<axum::Router>) = match nome {
        "discord" => {
            let (b, p) = ctx.base(Some(super::discord::BASE))?;
            let x = super::discord::Discord::novo(ctx.segredo("TOKEN")?, conversas, &b, p)?;
            (Arc::new(x), None)
        }
        "slack" => {
            let (b, p) = ctx.base(Some(super::slack::BASE))?;
            let x = super::slack::Slack::novo(ctx.segredo("TOKEN")?, conversas, &b, p)?;
            (Arc::new(x), None)
        }
        "whatsapp" | "messenger" => {
            let (b, p) = ctx.base(Some(super::meta::BASE))?;
            let assinatura = super::meta::Assinatura {
                app_secret: ctx.segredo("APP_SECRET")?,
                verify_token: ctx.segredo("VERIFY_TOKEN")?,
            };
            if nome == "whatsapp" {
                let x = Arc::new(super::meta::WhatsApp::novo(
                    ctx.caixa("")?,
                    assinatura,
                    ctx.segredo("TOKEN")?,
                    ctx.exigir("PHONE_ID")?,
                    &b,
                    p,
                )?);
                (x.clone(), recebedor(&x))
            } else {
                let x = Arc::new(super::meta::Messenger::novo(
                    ctx.caixa("")?,
                    assinatura,
                    ctx.segredo("TOKEN")?,
                    &b,
                    p,
                )?);
                (x.clone(), recebedor(&x))
            }
        }
        "teams" => {
            let login = ctx
                .cfg("LOGIN")
                .unwrap_or_else(|| super::teams::LOGIN.into());
            let mut p = politica_para(&login)?;
            for s in lista(
                &ctx.cfg("SERVICOS")
                    .unwrap_or_else(|| super::teams::SERVICO.into()),
            ) {
                let u = reqwest::Url::parse(&s).map_err(|e| format!("{s}: {e}"))?;
                p = p.with_origin(u.origin().ascii_serialization());
            }
            // O JWT RS256 e conferido sempre; a chave da URL so soma, se configurada.
            let jwks = super::jwt::Jwks::configurado(
                "teams",
                ctx.cfg("JWKS").as_deref(),
                super::teams::JWKS,
            )?;
            let x = Arc::new(super::teams::Teams::novo(
                ctx.caixa("")?,
                ctx.segredo_opcional("CHAVE_URL")?,
                ctx.segredo("APP_SECRET")?,
                ctx.exigir("APP_ID")?,
                &login,
                p,
                jwks,
            )?);
            (x.clone(), recebedor(&x))
        }
        "matrix" => {
            let (b, p) = ctx.base(None)?;
            let x =
                super::matrix::Matrix::novo(ctx.segredo("TOKEN")?, ctx.exigir("USUARIO")?, &b, p)?;
            (Arc::new(x), None)
        }
        "email" => {
            let mut smtp = crate::email::SmtpConfig::from_env().ok_or(
                "o canal de e-mail responde pelo SMTP de PHXCLAW_SMTP_* e PHXCLAW_EMAIL_FROM",
            )?;
            // A senha do SMTP sai da config do canal para o broker, como a do IMAP.
            let smtp_senha = ctx.segredo_de("SMTP_SENHA", smtp.password.take())?;
            let cfg = super::email::Config {
                smtp_senha,
                endereco: ctx.exigir("IMAP")?,
                usuario: ctx.exigir("USUARIO")?,
                senha: ctx.segredo("SENHA")?,
                pasta: ctx.cfg("PASTA").unwrap_or_else(|| "INBOX".into()),
                exigir_dmarc: ctx.cfg("EXIGIR_DMARC").as_deref() != Some("nao"),
                tls: ctx.tls()?,
            };
            (Arc::new(super::email::Email::novo(cfg, smtp)), None)
        }
        "webhook" => {
            let saida = ctx.cfg("SAIDA_URL");
            let politica = saida.as_deref().map(politica_para).transpose()?;
            let x = Arc::new(super::webhook::Webhook::novo(
                ctx.caixa("")?,
                ctx.segredo("SEGREDO")?,
                saida.as_deref().zip(politica),
            )?);
            (x.clone(), recebedor(&x))
        }
        "webchat" => {
            let x = Arc::new(super::webchat::Webchat::novo(
                ctx.caixa("")?,
                ctx.caixa("-saida")?,
                ctx.segredo("CHAVE")?,
            ));
            (x.clone(), Some(super::webchat::rotas(x)))
        }
        "signal" => {
            let (b, p) = ctx.base(Some("http://127.0.0.1:8080"))?;
            let x = super::signal::Signal::novo(
                ctx.caixa("")?,
                ctx.exigir("NUMERO")?,
                ctx.segredo_opcional("TOKEN")?,
                &b,
                p,
            )?;
            (Arc::new(x), None)
        }
        "googlechat" => {
            let (b, p) = ctx.base(Some(super::googlechat::BASE))?;
            // Sem audiencia nao ha token que se confira: o canal nao sobe, dizendo o que falta.
            let verificacao = super::googlechat::Verificacao::nova(
                ctx.exigir("AUDIENCIA")?,
                ctx.cfg("JWKS").as_deref(),
            )?;
            let x = Arc::new(super::googlechat::GoogleChat::novo(
                ctx.caixa("")?,
                ctx.segredo_opcional("CHAVE_URL")?,
                ctx.segredo("SAIDA_WEBHOOK")?,
                ctx.exigir("ESPACO")?,
                &b,
                p,
                verificacao,
            )?);
            (x.clone(), recebedor(&x))
        }
        "sms" => {
            let (b, p) = ctx.base(Some(super::sms::BASE))?;
            let x = Arc::new(super::sms::Sms::novo(
                ctx.caixa("")?,
                ctx.segredo("TOKEN")?,
                ctx.exigir("CONTA")?,
                ctx.exigir("NUMERO")?,
                ctx.exigir("URL_PUBLICA")?,
                &b,
                p,
            )?);
            (x.clone(), recebedor(&x))
        }
        "mattermost" => {
            let (b, p) = ctx.base(None)?;
            let x = super::mattermost::Mattermost::novo(
                ctx.segredo("TOKEN")?,
                ctx.exigir("BOT_ID")?,
                conversas,
                &b,
                p,
            )?;
            (Arc::new(x), None)
        }
        "rocketchat" => {
            let (b, p) = ctx.base(None)?;
            let x = super::rocketchat::RocketChat::novo(
                ctx.segredo("TOKEN")?,
                ctx.exigir("USUARIO_ID")?,
                ctx.cfg("TIPO").unwrap_or_else(|| "channels".into()),
                conversas,
                &b,
                p,
            )?;
            (Arc::new(x), None)
        }
        "zulip" => {
            let (b, p) = ctx.base(None)?;
            let x = super::zulip::Zulip::novo(ctx.segredo("CHAVE")?, ctx.exigir("EMAIL")?, &b, p)?;
            (Arc::new(x), None)
        }
        "irc" | "twitch" => {
            let cfg = super::irc::Config {
                nome: if nome == "twitch" { "twitch" } else { "irc" },
                endereco: ctx.exigir("ENDERECO")?,
                nick: ctx.exigir("NICK")?,
                senha: ctx.segredo_opcional("SENHA")?,
                salas: conversas
                    .iter()
                    .filter(|c| c.starts_with('#'))
                    .cloned()
                    .collect(),
                tls: ctx.tls()?,
            };
            (Arc::new(super::irc::Irc::novo(cfg, ctx.caixa("")?)), None)
        }
        "xmpp" => {
            let cfg = super::xmpp::Config {
                endereco: ctx.exigir("ENDERECO")?,
                jid: ctx.exigir("JID")?,
                senha: ctx.segredo("SENHA")?,
                tls: ctx.tls()?,
                salas: lista(&ctx.cfg("SALAS").unwrap_or_default())
                    .into_iter()
                    .collect(),
                apelido: ctx.cfg("APELIDO").unwrap_or_default(),
                permitidos: permitidos.iter().cloned().collect(),
                confiar_no_nick: sim_ou_nao(ctx.cfg("CONFIAR_NO_NICK"), false),
            };
            (Arc::new(super::xmpp::Xmpp::novo(cfg, ctx.caixa("")?)), None)
        }
        "mastodon" => {
            let (b, p) = ctx.base(None)?;
            (
                Arc::new(super::mastodon::Mastodon::novo(
                    ctx.segredo("TOKEN")?,
                    &b,
                    p,
                )?),
                None,
            )
        }
        "line" => {
            let (b, p) = ctx.base(Some(super::line::BASE))?;
            let x = Arc::new(super::line::Line::novo(
                ctx.caixa("")?,
                ctx.segredo("SEGREDO_CANAL")?,
                ctx.segredo("TOKEN")?,
                &b,
                p,
            )?);
            (x.clone(), recebedor(&x))
        }
        "viber" => {
            let (b, p) = ctx.base(Some(super::viber::BASE))?;
            let x = Arc::new(super::viber::Viber::novo(
                ctx.caixa("")?,
                ctx.segredo("TOKEN")?,
                ctx.cfg("REMETENTE").unwrap_or_else(|| "PhxClaw".into()),
                &b,
                p,
            )?);
            (x.clone(), recebedor(&x))
        }
        "feishu" => {
            let (b, p) = ctx.base(Some(super::feishu::BASE))?;
            let x = Arc::new(super::feishu::Feishu::novo(
                ctx.caixa("")?,
                ctx.segredo("VERIFICACAO")?,
                ctx.segredo("APP_SECRET")?,
                ctx.exigir("APP_ID")?,
                &b,
                p,
            )?);
            (x.clone(), recebedor(&x))
        }
        "reddit" => {
            let www = ctx.cfg("WWW").unwrap_or_else(|| super::reddit::WWW.into());
            let oauth = ctx
                .cfg("OAUTH")
                .unwrap_or_else(|| super::reddit::OAUTH.into());
            let u = reqwest::Url::parse(&oauth).map_err(|e| format!("{oauth}: {e}"))?;
            let p = politica_para(&www)?.with_origin(u.origin().ascii_serialization());
            let x = super::reddit::Reddit::novo(
                ctx.caixa("")?,
                ctx.segredo("APP_SECRET")?,
                ctx.segredo("SENHA")?,
                ctx.exigir("APP_ID")?,
                ctx.exigir("USUARIO")?,
                &www,
                &oauth,
                p,
            )?;
            (Arc::new(x), None)
        }
        "nostr" => {
            // A chave secreta (hex) vai ao broker; a lista de permitidos e de chaves
            // publicas em hex.
            let x = super::nostr::Nostr::novo(ctx.exigir("RELAY")?, ctx.segredo("CHAVE")?)?;
            (Arc::new(x), None)
        }
        outro => return Err(format!("canal sem montagem: {outro}")),
    };
    (log)(&format!(
        "{nome}: ligado ({} conversa(s) permitida(s){})",
        permitidos.len(),
        if rotas.is_some() {
            ", com entrada HTTP"
        } else {
            ""
        }
    ));
    let canal = Canal::de_arc(provedor, nome, permitidos, pasta, log)?;
    Ok(Ligado { canal, rotas })
}
