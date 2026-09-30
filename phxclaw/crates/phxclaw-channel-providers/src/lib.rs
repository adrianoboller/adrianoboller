#![forbid(unsafe_code)]
//! Secret-backed Telegram and Discord channel providers for PhxClaw F21/F23.
//! Tokens are resolved from short-lived broker leases and are never stored in manifests.

use phxclaw_channel_gateway::{
    ChannelCapabilities, ChannelProbe, ChannelProvider, ChannelProviderV2, OutboundMessage,
    ProviderReceipt,
};
use phxclaw_secret_broker::{SecretBroker, SecretBrokerError, SecretValue, scrub_text};
use reqwest::blocking::Client;
use serde::Deserialize;
use serde_json::{Value, json};
use std::{collections::BTreeSet, sync::Arc, time::Duration};
use thiserror::Error;
use url::Url;
use uuid::Uuid;

const TELEGRAM_DEFAULT_ORIGIN: &str = "https://api.telegram.org";
const DISCORD_DEFAULT_ORIGIN: &str = "https://discord.com";
const SLACK_DEFAULT_ORIGIN: &str = "https://slack.com";
const GRAPH_DEFAULT_ORIGIN: &str = "https://graph.microsoft.com";
const META_GRAPH_DEFAULT_ORIGIN: &str = "https://graph.facebook.com";

#[derive(Debug, Clone)]
pub struct ProviderEndpointPolicy {
    allowed_origins: BTreeSet<String>,
    allow_http: bool,
}

impl ProviderEndpointPolicy {
    #[must_use]
    pub fn locked_defaults() -> Self {
        Self {
            allowed_origins: [
                TELEGRAM_DEFAULT_ORIGIN.to_string(),
                DISCORD_DEFAULT_ORIGIN.to_string(),
                SLACK_DEFAULT_ORIGIN.to_string(),
                GRAPH_DEFAULT_ORIGIN.to_string(),
                META_GRAPH_DEFAULT_ORIGIN.to_string(),
            ]
            .into_iter()
            .collect(),
            allow_http: false,
        }
    }

    #[must_use]
    pub fn with_origin(mut self, origin: impl Into<String>) -> Self {
        self.allowed_origins.insert(origin.into());
        self
    }

    #[must_use]
    pub fn allow_http(mut self, allow: bool) -> Self {
        self.allow_http = allow;
        self
    }

    pub fn validate(&self, base: &str) -> Result<Url, ProviderError> {
        let url = Url::parse(base).map_err(|error| ProviderError::Endpoint(error.to_string()))?;
        if url.scheme() != "https" && !(self.allow_http && url.scheme() == "http") {
            return Err(ProviderError::Endpoint(
                "provider endpoint scheme is not allowed".into(),
            ));
        }
        let origin = url.origin().ascii_serialization();
        if !self.allowed_origins.contains(&origin) {
            return Err(ProviderError::Endpoint(format!(
                "provider origin not allowed: {origin}"
            )));
        }
        Ok(url)
    }
}

#[derive(Debug, Error)]
pub enum ProviderError {
    #[error(transparent)]
    Secret(#[from] SecretBrokerError),
    #[error("provider endpoint rejected: {0}")]
    Endpoint(String),
    #[error("provider request failed: {0}")]
    Request(String),
    #[error("provider returned non-success status {status}: {body}")]
    Http { status: u16, body: String },
    #[error("provider response invalid: {0}")]
    Response(String),
}

#[derive(Clone)]
pub struct TelegramProvider {
    broker: Arc<SecretBroker>,
    token_secret_uuid: Uuid,
    account_id: String,
    base_origin: String,
    policy: ProviderEndpointPolicy,
    client: Client,
}

impl TelegramProvider {
    pub fn new(
        broker: Arc<SecretBroker>,
        token_secret_uuid: Uuid,
        account_id: impl Into<String>,
    ) -> Result<Self, ProviderError> {
        Self::new_with_origin(
            broker,
            token_secret_uuid,
            account_id,
            TELEGRAM_DEFAULT_ORIGIN,
            ProviderEndpointPolicy::locked_defaults(),
        )
    }

    pub fn new_with_origin(
        broker: Arc<SecretBroker>,
        token_secret_uuid: Uuid,
        account_id: impl Into<String>,
        base_origin: impl Into<String>,
        policy: ProviderEndpointPolicy,
    ) -> Result<Self, ProviderError> {
        let base_origin = base_origin.into();
        policy.validate(&base_origin)?;
        let client = Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(30))
            .user_agent("PhxClaw/0.18 TelegramProvider")
            .build()
            .map_err(http_error)?;
        Ok(Self {
            broker,
            token_secret_uuid,
            account_id: account_id.into(),
            base_origin,
            policy,
            client,
        })
    }

    fn with_token<T>(
        &self,
        scope: &str,
        f: impl FnOnce(&SecretValue) -> Result<T, ProviderError>,
    ) -> Result<T, ProviderError> {
        let lease = self.broker.issue_lease(
            self.token_secret_uuid,
            "channel.provider.telegram",
            scope,
            30,
        )?;
        let token = self.broker.resolve(lease.uuid, scope)?;
        let result = f(&token);
        let _ = self.broker.revoke_lease(lease.uuid);
        result
    }

    fn send_impl(&self, message: &OutboundMessage) -> Result<ProviderReceipt, ProviderError> {
        self.policy.validate(&self.base_origin)?;
        self.with_token("channel:telegram:send", |token| {
            let endpoint = format!(
                "{}/bot{}/sendMessage",
                self.base_origin.trim_end_matches('/'),
                token.expose()
            );
            let response = self
                .client
                .post(endpoint)
                .json(&json!({
                    "chat_id": message.conversation_id,
                    "text": message.text,
                }))
                .send()
                .map_err(|error| {
                    ProviderError::Request(scrub_text(
                        &error.without_url().to_string(),
                        std::slice::from_ref(token),
                    ))
                })?;
            let status = response.status();
            let body = response.text().map_err(|error| {
                ProviderError::Request(scrub_text(
                    &error.without_url().to_string(),
                    std::slice::from_ref(token),
                ))
            })?;
            if !status.is_success() {
                return Err(ProviderError::Http {
                    status: status.as_u16(),
                    body: scrub_text(&body, std::slice::from_ref(token)),
                });
            }
            let parsed: TelegramResponse = serde_json::from_str(&body)
                .map_err(|error| ProviderError::Response(error.to_string()))?;
            if !parsed.ok {
                return Err(ProviderError::Response("Telegram returned ok=false".into()));
            }
            let result = parsed.result.ok_or_else(|| {
                ProviderError::Response("Telegram response missing result".into())
            })?;
            Ok(ProviderReceipt {
                provider_message_id: result.message_id.to_string(),
                metadata: json!({"provider":"telegram","account_id":self.account_id}),
            })
        })
    }

    fn probe_impl(&self) -> Result<ChannelProbe, ProviderError> {
        self.policy.validate(&self.base_origin)?;
        self.with_token("channel:telegram:probe", |token| {
            let endpoint = format!(
                "{}/bot{}/getMe",
                self.base_origin.trim_end_matches('/'),
                token.expose()
            );
            let response = self.client.get(endpoint).send().map_err(|error| {
                ProviderError::Request(scrub_text(
                    &error.without_url().to_string(),
                    std::slice::from_ref(token),
                ))
            })?;
            if !response.status().is_success() {
                return Ok(ChannelProbe {
                    connected: false,
                    account_id: None,
                    display_name: None,
                    error: Some(format!("HTTP {}", response.status().as_u16())),
                });
            }
            let body = response.text().map_err(http_error)?;
            let parsed: TelegramMeResponse = serde_json::from_str(&body)
                .map_err(|error| ProviderError::Response(error.to_string()))?;
            let me = parsed.result;
            Ok(ChannelProbe {
                connected: parsed.ok && me.is_some(),
                account_id: me.as_ref().map(|item| item.id.to_string()),
                display_name: me.and_then(|item| item.username.or(item.first_name)),
                error: None,
            })
        })
    }
}

impl ChannelProvider for TelegramProvider {
    fn channel(&self) -> &str {
        "telegram"
    }

    fn send(&self, message: &OutboundMessage) -> Result<ProviderReceipt, String> {
        self.send_impl(message).map_err(|error| error.to_string())
    }
}

impl ChannelProviderV2 for TelegramProvider {
    fn provider_id(&self) -> &str {
        "phxclaw.telegram.rest"
    }

    fn capabilities(&self) -> ChannelCapabilities {
        ChannelCapabilities {
            text: true,
            images: true,
            videos: true,
            voice: true,
            files: true,
            threads: true,
            reactions: true,
            editing: true,
            deletion: true,
        }
    }

    fn probe(&self) -> Result<ChannelProbe, String> {
        self.probe_impl().map_err(|error| error.to_string())
    }
}

#[derive(Clone)]
pub struct DiscordProvider {
    broker: Arc<SecretBroker>,
    token_secret_uuid: Uuid,
    account_id: String,
    base_origin: String,
    policy: ProviderEndpointPolicy,
    client: Client,
}

impl DiscordProvider {
    pub fn new(
        broker: Arc<SecretBroker>,
        token_secret_uuid: Uuid,
        account_id: impl Into<String>,
    ) -> Result<Self, ProviderError> {
        Self::new_with_origin(
            broker,
            token_secret_uuid,
            account_id,
            "https://discord.com/api/v10",
            ProviderEndpointPolicy::locked_defaults(),
        )
    }

    pub fn new_with_origin(
        broker: Arc<SecretBroker>,
        token_secret_uuid: Uuid,
        account_id: impl Into<String>,
        base_origin: impl Into<String>,
        policy: ProviderEndpointPolicy,
    ) -> Result<Self, ProviderError> {
        let base_origin = base_origin.into();
        policy.validate(&base_origin)?;
        let client = Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(30))
            .user_agent("PhxClaw/0.18 DiscordProvider")
            .build()
            .map_err(http_error)?;
        Ok(Self {
            broker,
            token_secret_uuid,
            account_id: account_id.into(),
            base_origin,
            policy,
            client,
        })
    }

    fn with_token<T>(
        &self,
        scope: &str,
        f: impl FnOnce(&SecretValue) -> Result<T, ProviderError>,
    ) -> Result<T, ProviderError> {
        let lease = self.broker.issue_lease(
            self.token_secret_uuid,
            "channel.provider.discord",
            scope,
            30,
        )?;
        let token = self.broker.resolve(lease.uuid, scope)?;
        let result = f(&token);
        let _ = self.broker.revoke_lease(lease.uuid);
        result
    }

    fn send_impl(&self, message: &OutboundMessage) -> Result<ProviderReceipt, ProviderError> {
        self.policy.validate(&self.base_origin)?;
        self.with_token("channel:discord:send", |token| {
            let endpoint = format!(
                "{}/channels/{}/messages",
                self.base_origin.trim_end_matches('/'),
                message.conversation_id
            );
            let response = self
                .client
                .post(endpoint)
                .header("Authorization", format!("Bot {}", token.expose()))
                .json(&json!({"content":message.text,"allowed_mentions":{"parse":[]}}))
                .send()
                .map_err(|error| {
                    ProviderError::Request(scrub_text(
                        &error.without_url().to_string(),
                        std::slice::from_ref(token),
                    ))
                })?;
            let status = response.status();
            let body = response.text().map_err(http_error)?;
            if !status.is_success() {
                return Err(ProviderError::Http {
                    status: status.as_u16(),
                    body: scrub_text(&body, std::slice::from_ref(token)),
                });
            }
            let parsed: DiscordMessage = serde_json::from_str(&body)
                .map_err(|error| ProviderError::Response(error.to_string()))?;
            Ok(ProviderReceipt {
                provider_message_id: parsed.id,
                metadata: json!({"provider":"discord","account_id":self.account_id}),
            })
        })
    }

    fn probe_impl(&self) -> Result<ChannelProbe, ProviderError> {
        self.policy.validate(&self.base_origin)?;
        self.with_token("channel:discord:probe", |token| {
            let endpoint = format!("{}/users/@me", self.base_origin.trim_end_matches('/'));
            let response = self
                .client
                .get(endpoint)
                .header("Authorization", format!("Bot {}", token.expose()))
                .send()
                .map_err(|error| {
                    ProviderError::Request(scrub_text(
                        &error.without_url().to_string(),
                        std::slice::from_ref(token),
                    ))
                })?;
            if !response.status().is_success() {
                return Ok(ChannelProbe {
                    connected: false,
                    account_id: None,
                    display_name: None,
                    error: Some(format!("HTTP {}", response.status().as_u16())),
                });
            }
            let me: DiscordUser = response
                .json()
                .map_err(|error| ProviderError::Response(error.to_string()))?;
            Ok(ChannelProbe {
                connected: true,
                account_id: Some(me.id),
                display_name: Some(me.username),
                error: None,
            })
        })
    }
}

impl ChannelProvider for DiscordProvider {
    fn channel(&self) -> &str {
        "discord"
    }

    fn send(&self, message: &OutboundMessage) -> Result<ProviderReceipt, String> {
        self.send_impl(message).map_err(|error| error.to_string())
    }
}

impl ChannelProviderV2 for DiscordProvider {
    fn provider_id(&self) -> &str {
        "phxclaw.discord.rest"
    }

    fn capabilities(&self) -> ChannelCapabilities {
        ChannelCapabilities {
            text: true,
            images: true,
            videos: true,
            voice: false,
            files: true,
            threads: true,
            reactions: true,
            editing: true,
            deletion: true,
        }
    }

    fn probe(&self) -> Result<ChannelProbe, String> {
        self.probe_impl().map_err(|error| error.to_string())
    }
}

#[derive(Clone)]
pub struct SlackProvider {
    broker: Arc<SecretBroker>,
    token_secret_uuid: Uuid,
    account_id: String,
    base_origin: String,
    policy: ProviderEndpointPolicy,
    client: Client,
}

impl SlackProvider {
    pub fn new(
        broker: Arc<SecretBroker>,
        token_secret_uuid: Uuid,
        account_id: impl Into<String>,
    ) -> Result<Self, ProviderError> {
        Self::new_with_origin(
            broker,
            token_secret_uuid,
            account_id,
            "https://slack.com/api",
            ProviderEndpointPolicy::locked_defaults(),
        )
    }

    pub fn new_with_origin(
        broker: Arc<SecretBroker>,
        token_secret_uuid: Uuid,
        account_id: impl Into<String>,
        base_origin: impl Into<String>,
        policy: ProviderEndpointPolicy,
    ) -> Result<Self, ProviderError> {
        let base_origin = base_origin.into();
        policy.validate(&base_origin)?;
        let client = Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(30))
            .user_agent("PhxClaw/0.68 SlackProvider")
            .build()
            .map_err(http_error)?;
        Ok(Self {
            broker,
            token_secret_uuid,
            account_id: account_id.into(),
            base_origin,
            policy,
            client,
        })
    }

    fn with_token<T>(
        &self,
        scope: &str,
        f: impl FnOnce(&SecretValue) -> Result<T, ProviderError>,
    ) -> Result<T, ProviderError> {
        let lease =
            self.broker
                .issue_lease(self.token_secret_uuid, "channel.provider.slack", scope, 30)?;
        let token = self.broker.resolve(lease.uuid, scope)?;
        let result = f(&token);
        let _ = self.broker.revoke_lease(lease.uuid);
        result
    }

    fn send_impl(&self, message: &OutboundMessage) -> Result<ProviderReceipt, ProviderError> {
        self.policy.validate(&self.base_origin)?;
        self.with_token("channel:slack:send", |token| {
            let endpoint = format!("{}/chat.postMessage", self.base_origin.trim_end_matches('/'));
            let response = self.client.post(endpoint)
                .bearer_auth(token.expose())
                .json(&json!({"channel":message.conversation_id,"text":message.text}))
                .send()
                .map_err(|error| ProviderError::Request(scrub_text(&error.without_url().to_string(), std::slice::from_ref(token))))?;
            let status = response.status();
            let body = response.text().map_err(http_error)?;
            if !status.is_success() {
                return Err(ProviderError::Http { status: status.as_u16(), body: scrub_text(&body, std::slice::from_ref(token)) });
            }
            let parsed: SlackMessageResponse = serde_json::from_str(&body).map_err(|error| ProviderError::Response(error.to_string()))?;
            if !parsed.ok { return Err(ProviderError::Response(parsed.error.unwrap_or_else(|| "Slack returned ok=false".into()))); }
            let ts = parsed.ts.ok_or_else(|| ProviderError::Response("Slack response missing ts".into()))?;
            Ok(ProviderReceipt { provider_message_id: ts, metadata: json!({"provider":"slack","account_id":self.account_id,"channel":parsed.channel}) })
        })
    }

    fn probe_impl(&self) -> Result<ChannelProbe, ProviderError> {
        self.policy.validate(&self.base_origin)?;
        self.with_token("channel:slack:probe", |token| {
            let endpoint = format!("{}/auth.test", self.base_origin.trim_end_matches('/'));
            let response = self
                .client
                .post(endpoint)
                .bearer_auth(token.expose())
                .send()
                .map_err(|error| {
                    ProviderError::Request(scrub_text(
                        &error.without_url().to_string(),
                        std::slice::from_ref(token),
                    ))
                })?;
            let status = response.status();
            let body = response.text().map_err(http_error)?;
            if !status.is_success() {
                return Ok(ChannelProbe {
                    connected: false,
                    account_id: None,
                    display_name: None,
                    error: Some(format!("HTTP {}", status.as_u16())),
                });
            }
            let parsed: SlackAuthResponse = serde_json::from_str(&body)
                .map_err(|error| ProviderError::Response(error.to_string()))?;
            Ok(ChannelProbe {
                connected: parsed.ok,
                account_id: parsed.team_id.or(parsed.user_id),
                display_name: parsed.user.or(parsed.team),
                error: parsed.error,
            })
        })
    }
}

impl ChannelProvider for SlackProvider {
    fn channel(&self) -> &str {
        "slack"
    }
    fn send(&self, message: &OutboundMessage) -> Result<ProviderReceipt, String> {
        self.send_impl(message).map_err(|error| error.to_string())
    }
}
impl ChannelProviderV2 for SlackProvider {
    fn provider_id(&self) -> &str {
        "phxclaw.slack.web-api"
    }
    fn capabilities(&self) -> ChannelCapabilities {
        ChannelCapabilities {
            text: true,
            images: true,
            videos: true,
            voice: true,
            files: true,
            threads: true,
            reactions: true,
            editing: true,
            deletion: true,
        }
    }
    fn probe(&self) -> Result<ChannelProbe, String> {
        self.probe_impl().map_err(|error| error.to_string())
    }
}

#[derive(Clone)]
pub struct WhatsAppProvider {
    broker: Arc<SecretBroker>,
    token_secret_uuid: Uuid,
    phone_number_id: String,
    graph_version: String,
    base_origin: String,
    policy: ProviderEndpointPolicy,
    client: Client,
}

impl WhatsAppProvider {
    pub fn new(
        broker: Arc<SecretBroker>,
        token_secret_uuid: Uuid,
        phone_number_id: impl Into<String>,
        graph_version: impl Into<String>,
    ) -> Result<Self, ProviderError> {
        Self::new_with_origin(
            broker,
            token_secret_uuid,
            phone_number_id,
            graph_version,
            META_GRAPH_DEFAULT_ORIGIN,
            ProviderEndpointPolicy::locked_defaults(),
        )
    }
    pub fn new_with_origin(
        broker: Arc<SecretBroker>,
        token_secret_uuid: Uuid,
        phone_number_id: impl Into<String>,
        graph_version: impl Into<String>,
        base_origin: impl Into<String>,
        policy: ProviderEndpointPolicy,
    ) -> Result<Self, ProviderError> {
        let base_origin = base_origin.into();
        policy.validate(&base_origin)?;
        let client = Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(30))
            .user_agent("PhxClaw/0.68 WhatsAppProvider")
            .build()
            .map_err(http_error)?;
        Ok(Self {
            broker,
            token_secret_uuid,
            phone_number_id: phone_number_id.into(),
            graph_version: graph_version.into(),
            base_origin,
            policy,
            client,
        })
    }
    fn with_token<T>(
        &self,
        scope: &str,
        f: impl FnOnce(&SecretValue) -> Result<T, ProviderError>,
    ) -> Result<T, ProviderError> {
        let lease = self.broker.issue_lease(
            self.token_secret_uuid,
            "channel.provider.whatsapp",
            scope,
            30,
        )?;
        let token = self.broker.resolve(lease.uuid, scope)?;
        let result = f(&token);
        let _ = self.broker.revoke_lease(lease.uuid);
        result
    }
    fn endpoint(&self, suffix: &str) -> String {
        format!(
            "{}/{}/{}{}",
            self.base_origin.trim_end_matches('/'),
            self.graph_version.trim_matches('/'),
            self.phone_number_id,
            suffix
        )
    }
    fn send_impl(&self, message: &OutboundMessage) -> Result<ProviderReceipt, ProviderError> {
        self.policy.validate(&self.base_origin)?;
        self.with_token("channel:whatsapp:send",|token|{
            let response=self.client.post(self.endpoint("/messages")).bearer_auth(token.expose())
                .json(&json!({"messaging_product":"whatsapp","recipient_type":"individual","to":message.conversation_id,"type":"text","text":{"preview_url":false,"body":message.text}}))
                .send().map_err(|error|ProviderError::Request(scrub_text(&error.without_url().to_string(),std::slice::from_ref(token))))?;
            let status=response.status(); let body=response.text().map_err(http_error)?;
            if !status.is_success(){return Err(ProviderError::Http{status:status.as_u16(),body:scrub_text(&body,std::slice::from_ref(token))});}
            let parsed:WhatsAppMessageResponse=serde_json::from_str(&body).map_err(|error|ProviderError::Response(error.to_string()))?;
            let id=parsed.messages.and_then(|mut x|x.pop()).map(|x|x.id).ok_or_else(||ProviderError::Response("WhatsApp response missing message id".into()))?;
            Ok(ProviderReceipt{provider_message_id:id,metadata:json!({"provider":"whatsapp","phone_number_id":self.phone_number_id})})
        })
    }
    fn probe_impl(&self) -> Result<ChannelProbe, ProviderError> {
        self.policy.validate(&self.base_origin)?;
        self.with_token("channel:whatsapp:probe", |token| {
            let url = format!(
                "{}?fields=id,display_phone_number,verified_name",
                self.endpoint("")
            );
            let response = self
                .client
                .get(url)
                .bearer_auth(token.expose())
                .send()
                .map_err(|error| {
                    ProviderError::Request(scrub_text(
                        &error.without_url().to_string(),
                        std::slice::from_ref(token),
                    ))
                })?;
            if !response.status().is_success() {
                return Ok(ChannelProbe {
                    connected: false,
                    account_id: None,
                    display_name: None,
                    error: Some(format!("HTTP {}", response.status().as_u16())),
                });
            }
            let parsed: WhatsAppPhone = response
                .json()
                .map_err(|error| ProviderError::Response(error.to_string()))?;
            Ok(ChannelProbe {
                connected: true,
                account_id: Some(parsed.id),
                display_name: parsed.verified_name.or(parsed.display_phone_number),
                error: None,
            })
        })
    }
}
impl ChannelProvider for WhatsAppProvider {
    fn channel(&self) -> &str {
        "whatsapp"
    }
    fn send(&self, message: &OutboundMessage) -> Result<ProviderReceipt, String> {
        self.send_impl(message).map_err(|e| e.to_string())
    }
}
impl ChannelProviderV2 for WhatsAppProvider {
    fn provider_id(&self) -> &str {
        "phxclaw.whatsapp.cloud-api"
    }
    fn capabilities(&self) -> ChannelCapabilities {
        ChannelCapabilities {
            text: true,
            images: true,
            videos: true,
            voice: true,
            files: true,
            threads: false,
            reactions: true,
            editing: true,
            deletion: false,
        }
    }
    fn probe(&self) -> Result<ChannelProbe, String> {
        self.probe_impl().map_err(|e| e.to_string())
    }
}

#[derive(Clone, Debug)]
pub enum TeamsRoute {
    Chat,
    Channel { team_id: String },
}

#[derive(Clone)]
pub struct TeamsProvider {
    broker: Arc<SecretBroker>,
    token_secret_uuid: Uuid,
    account_id: String,
    route: TeamsRoute,
    base_origin: String,
    policy: ProviderEndpointPolicy,
    client: Client,
}
impl TeamsProvider {
    pub fn new(
        broker: Arc<SecretBroker>,
        token_secret_uuid: Uuid,
        account_id: impl Into<String>,
        route: TeamsRoute,
    ) -> Result<Self, ProviderError> {
        Self::new_with_origin(
            broker,
            token_secret_uuid,
            account_id,
            route,
            "https://graph.microsoft.com/v1.0",
            ProviderEndpointPolicy::locked_defaults(),
        )
    }
    pub fn new_with_origin(
        broker: Arc<SecretBroker>,
        token_secret_uuid: Uuid,
        account_id: impl Into<String>,
        route: TeamsRoute,
        base_origin: impl Into<String>,
        policy: ProviderEndpointPolicy,
    ) -> Result<Self, ProviderError> {
        let base_origin = base_origin.into();
        policy.validate(&base_origin)?;
        let client = Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(30))
            .user_agent("PhxClaw/0.68 TeamsProvider")
            .build()
            .map_err(http_error)?;
        Ok(Self {
            broker,
            token_secret_uuid,
            account_id: account_id.into(),
            route,
            base_origin,
            policy,
            client,
        })
    }
    fn with_token<T>(
        &self,
        scope: &str,
        f: impl FnOnce(&SecretValue) -> Result<T, ProviderError>,
    ) -> Result<T, ProviderError> {
        let lease =
            self.broker
                .issue_lease(self.token_secret_uuid, "channel.provider.teams", scope, 30)?;
        let token = self.broker.resolve(lease.uuid, scope)?;
        let result = f(&token);
        let _ = self.broker.revoke_lease(lease.uuid);
        result
    }
    fn send_impl(&self, message: &OutboundMessage) -> Result<ProviderReceipt, ProviderError> {
        self.policy.validate(&self.base_origin)?;
        self.with_token("channel:teams:send", |token| {
            let endpoint = match &self.route {
                TeamsRoute::Chat => format!(
                    "{}/chats/{}/messages",
                    self.base_origin.trim_end_matches('/'),
                    message.conversation_id
                ),
                TeamsRoute::Channel { team_id } => format!(
                    "{}/teams/{}/channels/{}/messages",
                    self.base_origin.trim_end_matches('/'),
                    team_id,
                    message.conversation_id
                ),
            };
            let response = self
                .client
                .post(endpoint)
                .bearer_auth(token.expose())
                .json(&json!({"body":{"contentType":"text","content":message.text}}))
                .send()
                .map_err(|e| {
                    ProviderError::Request(scrub_text(
                        &e.without_url().to_string(),
                        std::slice::from_ref(token),
                    ))
                })?;
            let status = response.status();
            let body = response.text().map_err(http_error)?;
            if !status.is_success() {
                return Err(ProviderError::Http {
                    status: status.as_u16(),
                    body: scrub_text(&body, std::slice::from_ref(token)),
                });
            }
            let parsed: TeamsMessage =
                serde_json::from_str(&body).map_err(|e| ProviderError::Response(e.to_string()))?;
            Ok(ProviderReceipt {
                provider_message_id: parsed.id,
                metadata: json!({"provider":"teams","account_id":self.account_id}),
            })
        })
    }
    fn probe_impl(&self) -> Result<ChannelProbe, ProviderError> {
        self.policy.validate(&self.base_origin)?;
        self.with_token("channel:teams:probe", |token| {
            let response = self
                .client
                .get(format!("{}/me", self.base_origin.trim_end_matches('/')))
                .bearer_auth(token.expose())
                .send()
                .map_err(|e| {
                    ProviderError::Request(scrub_text(
                        &e.without_url().to_string(),
                        std::slice::from_ref(token),
                    ))
                })?;
            if !response.status().is_success() {
                return Ok(ChannelProbe {
                    connected: false,
                    account_id: None,
                    display_name: None,
                    error: Some(format!("HTTP {}", response.status().as_u16())),
                });
            }
            let me: TeamsMe = response
                .json()
                .map_err(|e| ProviderError::Response(e.to_string()))?;
            Ok(ChannelProbe {
                connected: true,
                account_id: Some(me.id),
                display_name: me.display_name.or(me.user_principal_name),
                error: None,
            })
        })
    }
}
impl ChannelProvider for TeamsProvider {
    fn channel(&self) -> &str {
        "teams"
    }
    fn send(&self, message: &OutboundMessage) -> Result<ProviderReceipt, String> {
        self.send_impl(message).map_err(|e| e.to_string())
    }
}
impl ChannelProviderV2 for TeamsProvider {
    fn provider_id(&self) -> &str {
        "phxclaw.teams.graph"
    }
    fn capabilities(&self) -> ChannelCapabilities {
        ChannelCapabilities {
            text: true,
            images: true,
            videos: true,
            voice: false,
            files: true,
            threads: true,
            reactions: true,
            editing: true,
            deletion: true,
        }
    }
    fn probe(&self) -> Result<ChannelProbe, String> {
        self.probe_impl().map_err(|e| e.to_string())
    }
}

#[derive(Debug, Deserialize)]
struct SlackMessageResponse {
    ok: bool,
    ts: Option<String>,
    channel: Option<String>,
    error: Option<String>,
}
#[derive(Debug, Deserialize)]
struct SlackAuthResponse {
    ok: bool,
    team: Option<String>,
    team_id: Option<String>,
    user: Option<String>,
    user_id: Option<String>,
    error: Option<String>,
}
#[derive(Debug, Deserialize)]
struct WhatsAppMessageResponse {
    messages: Option<Vec<WhatsAppMessageId>>,
}
#[derive(Debug, Deserialize)]
struct WhatsAppMessageId {
    id: String,
}
#[derive(Debug, Deserialize)]
struct WhatsAppPhone {
    id: String,
    display_phone_number: Option<String>,
    verified_name: Option<String>,
}
#[derive(Debug, Deserialize)]
struct TeamsMessage {
    id: String,
}
#[derive(Debug, Deserialize)]
struct TeamsMe {
    id: String,
    #[serde(rename = "displayName")]
    display_name: Option<String>,
    #[serde(rename = "userPrincipalName")]
    user_principal_name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct TelegramResponse {
    ok: bool,
    result: Option<TelegramMessage>,
}

#[derive(Debug, Deserialize)]
struct TelegramMessage {
    message_id: i64,
}

#[derive(Debug, Deserialize)]
struct TelegramMeResponse {
    ok: bool,
    result: Option<TelegramUser>,
}

#[derive(Debug, Deserialize)]
struct TelegramUser {
    id: i64,
    username: Option<String>,
    first_name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DiscordMessage {
    id: String,
}

#[derive(Debug, Deserialize)]
struct DiscordUser {
    id: String,
    username: String,
}

pub fn provider_metadata() -> Value {
    json!({
        "telegram": {
            "provider_id": "phxclaw.telegram.rest",
            "default_origin": TELEGRAM_DEFAULT_ORIGIN,
            "secret_scopes": ["channel:telegram:send", "channel:telegram:probe"]
        },
        "discord": {
            "provider_id": "phxclaw.discord.rest",
            "default_origin": "https://discord.com",
            "secret_scopes": ["channel:discord:send", "channel:discord:probe"]
        },
        "slack": {
            "provider_id": "phxclaw.slack.web-api",
            "default_origin": "https://slack.com",
            "secret_scopes": ["channel:slack:send", "channel:slack:probe"]
        },
        "whatsapp": {
            "provider_id": "phxclaw.whatsapp.cloud-api",
            "default_origin": "https://graph.facebook.com",
            "secret_scopes": ["channel:whatsapp:send", "channel:whatsapp:probe"]
        },
        "teams": {
            "provider_id": "phxclaw.teams.graph",
            "default_origin": "https://graph.microsoft.com",
            "secret_scopes": ["channel:teams:send", "channel:teams:probe"]
        }
    })
}

/// Todo erro HTTP deste crate passa por aqui. O reqwest escreve a URL no Display do
/// erro, e a URL do Telegram carrega o token (`/bot<TOKEN>/...`): um corpo cortado no
/// meio vazava o token inteiro. Tirar a URL na origem cobre todo caminho de erro, inclusive
/// o que alguem escrever amanha sem lembrar do scrub_text.
fn http_error(error: reqwest::Error) -> ProviderError {
    ProviderError::Request(error.without_url().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use phxclaw_evidence_ledger::EvidenceLedger;
    use phxclaw_live_bus::LiveEventHub;
    use phxclaw_secret_broker::{FileMasterKeyProvider, SecretValue};
    use std::io::{Read, Write};
    use std::net::TcpListener;

    const TOKEN: &str = "123456:TOKEN-QUE-NAO-PODE-VAZAR";

    /// Servidor que responde 200, promete 1000 bytes e fecha depois de 10: o corpo cai
    /// no meio, e o erro do reqwest sai com a URL -- que no Telegram carrega o token.
    fn servidor_que_corta_o_corpo() -> String {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = l.local_addr().unwrap();
        std::thread::spawn(move || {
            if let Ok((mut s, _)) = l.accept() {
                let mut buf = [0u8; 4096];
                let _ = s.read(&mut buf);
                let _ = s.write_all(
                    b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 1000\r\n\r\n{\"ok\":true",
                );
            }
        });
        format!("http://{addr}")
    }

    #[test]
    fn corpo_cortado_no_probe_nao_vaza_o_token() {
        let dir = std::env::temp_dir().join(format!("phx-chan-{}", uuid::Uuid::now_v7()));
        let key = Arc::new(FileMasterKeyProvider::new(dir.join("master.key")));
        key.ensure().unwrap();
        let broker = Arc::new(
            SecretBroker::new(
                dir.join("secrets"),
                key,
                LiveEventHub::new(16, 16),
                EvidenceLedger::open(dir.join("evidence.jsonl")).unwrap(),
            )
            .unwrap(),
        );
        let d = broker
            .store(
                "telegram",
                "channels",
                vec!["*".into()],
                SecretValue::new(TOKEN.into()),
            )
            .unwrap();
        let origem = servidor_que_corta_o_corpo();
        let p = TelegramProvider::new_with_origin(
            broker,
            d.uuid,
            "conta",
            origem.clone(),
            ProviderEndpointPolicy::locked_defaults()
                .with_origin(origem)
                .allow_http(true),
        )
        .unwrap();
        let erro = ChannelProviderV2::probe(&p).expect_err("corpo cortado tem de falhar");
        assert!(
            !erro.contains("TOKEN-QUE-NAO-PODE-VAZAR"),
            "token vazou no erro: {erro}"
        );
        let _ = std::fs::remove_dir_all(dir);
    }
}
