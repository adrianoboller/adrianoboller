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
use serde_json::{Value, json};

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
        let v = |k: &str| std::env::var(k).ok().filter(|s| !s.trim().is_empty());
        let security = match v("PHXCLAW_SMTP_SECURITY").as_deref() {
            Some("plain") => SmtpSecurity::Plain,
            Some("starttls") => SmtpSecurity::StartTls,
            _ => SmtpSecurity::Tls,
        };
        Some(Self {
            host: v("PHXCLAW_SMTP_HOST")?,
            port: v("PHXCLAW_SMTP_PORT")
                .and_then(|p| p.parse().ok())
                .unwrap_or(match security {
                    SmtpSecurity::Tls => 465,
                    SmtpSecurity::StartTls => 587,
                    SmtpSecurity::Plain => 25,
                }),
            security,
            username: v("PHXCLAW_SMTP_USER"),
            password: v("PHXCLAW_SMTP_PASSWORD"),
            from: v("PHXCLAW_EMAIL_FROM")?,
            allowed_recipients: v("PHXCLAW_EMAIL_PERMITIDOS")
                .map(|s| {
                    s.split(',')
                        .map(|x| x.trim().to_ascii_lowercase())
                        .filter(|x| !x.is_empty())
                        .collect()
                })
                .unwrap_or_default(),
        })
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
            let c = &self.config;
            let loopback = matches!(c.host.as_str(), "127.0.0.1" | "localhost" | "::1");
            let mut t = match c.security {
                SmtpSecurity::Tls => AsyncSmtpTransport::<Tokio1Executor>::relay(&c.host),
                SmtpSecurity::StartTls => {
                    AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&c.host)
                }
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
            let r = t
                .build()
                .send(msg)
                .await
                // o erro do lettre nao carrega a senha; mesmo assim so o texto do servidor volta
                .map_err(|e| ToolError::Failed(format!("smtp: {e}")))?;
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
