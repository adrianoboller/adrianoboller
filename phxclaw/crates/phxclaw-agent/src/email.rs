//! Envio de e-mail pelo agente (o "Mail" do Manus), por SMTP.
//!
//! Enviar e-mail em nome de alguem e acao externa e irreversivel: a capacidade `mail.send`
//! NAO esta no padrao, os destinatarios passam por lista explicita (dominio ou endereco) e a
//! senha do SMTP so vive no transporte -- nunca volta ao modelo nem ao registro.

use crate::motor::sha256_hex;
use crate::tarefa::confine;
use lettre::message::{Attachment, Mailbox, MultiPart, SinglePart, header::ContentType};
use lettre::transport::smtp::authentication::Credentials;
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};
use phxclaw_agent_core::{BoxFut, Tool, ToolContext, ToolError, ToolOutput, ToolSpec};
use phxclaw_secret_broker::{SecretBroker, SecretValue};
use serde_json::{Value, json};
use std::path::Path;
use std::sync::Arc;

#[derive(Clone)]
pub struct SmtpConfig {
    pub host: String,
    pub port: u16,
    /// smtps (TLS direto), starttls, ou plain -- plain so e aceito em loopback.
    pub security: SmtpSecurity,
    pub username: Option<String>,
    pub password: Option<String>,
    pub from: String,
    /// Destinos permitidos: "fulano@x.com" ou "@x.com" (dominio inteiro).
    pub allowed_recipients: Vec<String>,
}

impl std::fmt::Debug for SmtpConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SmtpConfig")
            .field("host", &self.host)
            .field("port", &self.port)
            .field("from", &self.from)
            .field("password", &"[REDACTED]")
            .finish()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SmtpSecurity {
    Tls,
    StartTls,
    Plain,
}

impl SmtpConfig {
    /// PHXCLAW_SMTP_HOST, _PORT, _SECURITY (tls|starttls|plain), _USER, _PASSWORD,
    /// PHXCLAW_EMAIL_FROM e PHXCLAW_EMAIL_PERMITIDOS (lista separada por virgula).
    pub fn from_env() -> Option<Self> {
        let v = crate::config::texto_de;
        let security = match v("email.smtp.seguranca").as_deref() {
            Some("plain") => SmtpSecurity::Plain,
            Some("starttls") => SmtpSecurity::StartTls,
            _ => SmtpSecurity::Tls,
        };
        Some(Self {
            host: v("email.smtp.host")?,
            port: crate::config::inteiro_de("email.smtp.porta")
                .and_then(|p| u16::try_from(p).ok())
                .unwrap_or(match security {
                    SmtpSecurity::Tls => 465,
                    SmtpSecurity::StartTls => 587,
                    SmtpSecurity::Plain => 25,
                }),
            security,
            username: v("email.smtp.usuario"),
            password: crate::config::segredo_do_ambiente("email.smtp.senha")
                .filter(|s| !s.is_empty()),
            from: v("email.remetente")?,
            allowed_recipients: crate::config::lista_de("email.permitidos")
                .map(|l| {
                    l.iter()
                        .map(|x| x.trim().to_ascii_lowercase())
                        .filter(|x| !x.is_empty())
                        .collect()
                })
                .unwrap_or_default(),
        })
    }

    /// `from_env` e, sem `PHXCLAW_SMTP_PASSWORD` no ambiente, a senha guardada por
    /// `phxclaw email chave` -- o MESMO segredo (`email-smtp_senha`, broker do canal) que o
    /// canal de e-mail le pelo `segredo_de`. A ferramenta `send_email` da montagem passa
    /// por aqui; antes so olhava o ambiente, e a ajuda nao dizia.
    pub fn da_pasta(raiz_do_agente: &Path) -> Option<Self> {
        let mut c = Self::from_env()?;
        if c.password.is_none() {
            c.password = senha_smtp_guardada(raiz_do_agente).ok().flatten();
        }
        Some(c)
    }

    fn permitido(&self, to: &str) -> bool {
        let to = to.trim().to_ascii_lowercase();
        self.allowed_recipients.iter().any(|p| {
            if p.starts_with('@') {
                to.ends_with(p.as_str())
            } else {
                *p == to
            }
        })
    }
}

/// O nome e o canal do segredo da senha do SMTP, os mesmos do `ligar.rs` (`SMTP_SENHA` do
/// canal `email`): uma grafia so, para o comando e o canal nunca gravarem dois segredos.
const SEGREDO_SMTP: (&str, &str) = ("email-smtp_senha", "email");

fn broker_dos_canais(
    raiz_do_agente: &Path,
    criar: bool,
) -> Result<Option<Arc<SecretBroker>>, String> {
    let pasta = raiz_do_agente.join("canal");
    if !criar && !pasta.join("segredos/master.key").exists() {
        return Ok(None);
    }
    crate::canais::broker_em(&pasta).map(Some)
}

/// `phxclaw email chave`: `PHXCLAW_SMTP_PASSWORD` do ambiente do comando vai para o broker
/// do canal, com o nome que o canal de e-mail ja le.
pub fn guardar_senha_smtp(raiz_do_agente: &Path) -> Result<uuid::Uuid, String> {
    let variavel = crate::config::catalogo_do_config::por_chave("email.smtp.senha")
        .map(|c| c.variavel.clone())
        .ok_or("email.smtp.senha fora do catalogo")?;
    let senha = std::env::var(&variavel)
        .ok()
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| format!("defina {variavel} com a senha do SMTP"))?;
    let broker = broker_dos_canais(raiz_do_agente, true)?.ok_or("sem broker")?;
    crate::canais::guardar_do_canal(
        &broker,
        SEGREDO_SMTP.0,
        SEGREDO_SMTP.1,
        SecretValue::new(senha.trim().to_string()),
    )
}

fn senha_smtp_guardada(raiz_do_agente: &Path) -> Result<Option<String>, String> {
    let Some(broker) = broker_dos_canais(raiz_do_agente, false)? else {
        return Ok(None);
    };
    match crate::canais::segredo_guardado(&broker, SEGREDO_SMTP.0, "canais")? {
        None => Ok(None),
        Some(d) => crate::canais::http::Credencial::nova(broker, d.uuid, SEGREDO_SMTP.1)
            .com("send", |s| Ok(Some(s.to_string()))),
    }
}

impl SmtpConfig {
    /// O unico caminho de saida por SMTP: a ferramenta `send_email` e o canal de e-mail
    /// passam por aqui, para a regra "sem TLS so em loopback" nao existir em duas copias.
    pub async fn mandar(
        &self,
        msg: Message,
    ) -> Result<lettre::transport::smtp::response::Response, ToolError> {
        let c = self;
        let loopback = matches!(c.host.as_str(), "127.0.0.1" | "localhost" | "::1");
        let mut t = match c.security {
            SmtpSecurity::Tls => AsyncSmtpTransport::<Tokio1Executor>::relay(&c.host),
            SmtpSecurity::StartTls => AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&c.host),
            // Sem TLS so em loopback: senha em texto claro na rede seria vazamento.
            SmtpSecurity::Plain if loopback => Ok(
                AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(&c.host),
            ),
            SmtpSecurity::Plain => {
                return Err(ToolError::Denied("SMTP sem TLS so em loopback".into()));
            }
        }
        .map_err(|e| ToolError::Failed(format!("smtp: {e}")))?
        .port(c.port)
        .timeout(Some(std::time::Duration::from_secs(30)));
        if let (Some(u), Some(p)) = (&c.username, &c.password) {
            t = t.credentials(Credentials::new(u.clone(), p.clone()));
        }
        t.build()
            .send(msg)
            .await
            // o erro do lettre nao carrega a senha; mesmo assim so o texto do servidor volta
            .map_err(|e| ToolError::Failed(format!("smtp: {e}")))
    }
}

pub struct EmailTool {
    pub config: SmtpConfig,
}

impl Tool for EmailTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "send_email".into(),
            description: "Send an e-mail (plain text body) with optional attachments from the task directory. Only allowed recipients.".into(),
            parameters: json!({"type":"object","properties":{"to":{"type":"array","items":{"type":"string"}},"subject":{"type":"string"},"body":{"type":"string"},"attachments":{"type":"array","items":{"type":"string"},"description":"relative paths"}},"required":["to","subject","body"]}),
        }
    }
    fn capability(&self) -> &'static str {
        "mail.send"
    }
    fn run<'a>(
        &'a self,
        args: Value,
        ctx: &'a ToolContext,
    ) -> BoxFut<'a, Result<ToolOutput, ToolError>> {
        Box::pin(async move {
            let inv = |m: String| ToolError::InvalidArguments(m);
            let to: Vec<String> = args
                .get("to")
                .and_then(Value::as_array)
                .ok_or_else(|| inv("falta 'to' (lista)".into()))?
                .iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect();
            if to.is_empty() || to.len() > 20 {
                return Err(inv("de 1 a 20 destinatarios".into()));
            }
            if let Some(proibido) = to.iter().find(|t| !self.config.permitido(t)) {
                return Err(ToolError::Denied(format!(
                    "destinatario fora da lista permitida: {proibido}"
                )));
            }
            let subject = args
                .get("subject")
                .and_then(Value::as_str)
                .ok_or_else(|| inv("falta 'subject'".into()))?;
            let body = args
                .get("body")
                .and_then(Value::as_str)
                .ok_or_else(|| inv("falta 'body'".into()))?;
            let from: Mailbox = self
                .config
                .from
                .parse()
                .map_err(|e| ToolError::Failed(format!("remetente: {e}")))?;
            let mut b = Message::builder().from(from).subject(subject);
            for t in &to {
                b = b.to(t.parse().map_err(|e| inv(format!("endereco {t}: {e}")))?);
            }
            let mut partes = MultiPart::mixed().singlepart(SinglePart::plain(body.to_string()));
            let mut anexos = vec![];
            for rel in args
                .get("attachments")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default()
            {
                let rel = rel
                    .as_str()
                    .ok_or_else(|| inv("anexo deve ser caminho".into()))?
                    .to_string();
                let caminho = confine(&ctx.workdir, &rel).map_err(ToolError::Denied)?;
                let bytes = std::fs::read(&caminho)
                    .map_err(|e| ToolError::Failed(format!("{rel}: {e}")))?;
                let tipo = ContentType::parse(crate::motor::media_type(&rel))
                    .unwrap_or(ContentType::TEXT_PLAIN);
                let nome = rel.rsplit('/').next().unwrap_or(&rel).to_string();
                anexos.push(format!("{rel} (sha256 {}...)", &sha256_hex(&bytes)[..16]));
                partes = partes.singlepart(Attachment::new(nome).body(bytes, tipo));
            }
            let msg = b
                .multipart(partes)
                .map_err(|e| ToolError::Failed(e.to_string()))?;
            let r = self.config.mandar(msg).await?;
            Ok(ToolOutput::text(format!(
                "enviado para {} (codigo {}){}",
                to.join(", "),
                r.code(),
                if anexos.is_empty() {
                    String::new()
                } else {
                    format!("; anexos: {}", anexos.join(", "))
                }
            )))
        })
    }
}
