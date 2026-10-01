//! Telegram como canal do agente: a mensagem de um chat permitido vira tarefa, e a resposta
//! final volta ao mesmo chat.
//!
//! Decisoes que valem saber:
//! - o token mora so no SecretBroker (envelope cifrado) e e lido por concessao curta a cada
//!   chamada; nenhuma linha daqui o recebe, entao nenhum registro daqui consegue imprimi-lo;
//! - a tarefa nasce por `api::criar_tarefa`, a mesma funcao da rota HTTP: validacao, modelo
//!   padrao e balde de fichas sao os mesmos nas duas portas;
//! - a lista de chats permitidos e UMA decisao (`permitido`), usada pela entrada e pela
//!   ferramenta `channel_send`; chat fora dela nem chega ao gateway;
//! - o offset grava depois de a tarefa estar gravada: cair entre as duas coisas reprocessa
//!   uma mensagem (tarefa duplicada) em vez de perder uma (pedido sem resposta);
//! - o provedor e HTTP bloqueante, entao toda chamada a ele sai em `spawn_blocking`.

use crate::api::{ApiState, NovaTarefa, criar_tarefa};
use crate::tarefa::{Task, TaskStatus};
use chrono::Utc;
use phxclaw_agent_core::{BoxFut, Tool, ToolContext, ToolError, ToolOutput, ToolSpec};
use phxclaw_channel_gateway::{
    ChannelGateway, ChannelGatewayError, IdentityState, InboundMessage, OutboundMessage,
};
use phxclaw_channel_providers::{TELEGRAM_MAX_TEXT, TelegramProvider, TelegramUpdate};
use phxclaw_evidence_ledger::EvidenceLedger;
use phxclaw_live_bus::LiveEventHub;
use phxclaw_secret_broker::{FileMasterKeyProvider, SecretBroker, SecretValue};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use uuid::Uuid;

/// Para onde vao as linhas de registro do canal (terminal no binario, vetor no teste).
pub type Registro = Arc<dyn Fn(&str) + Send + Sync>;

const CANAL: &str = "telegram";
const NOME_SEGREDO: &str = "telegram-bot";
const ESCOPOS: &[&str] = &[
    "channel:telegram:send",
    "channel:telegram:probe",
    "channel:telegram:receive",
];

/// Broker de segredos da pasta do agente; a chave mestra nasce 0600 na primeira vez.
pub fn broker_em(pasta: &Path) -> Result<Arc<SecretBroker>, String> {
    let dir = pasta.join("segredos");
    let chave = Arc::new(FileMasterKeyProvider::new(dir.join("master.key")));
    chave.ensure().map_err(|e| e.to_string())?;
    let ledger = EvidenceLedger::open(dir.join("evidence.jsonl")).map_err(|e| e.to_string())?;
    SecretBroker::new(dir.join("cofre"), chave, LiveEventHub::new(16, 16), ledger)
        .map(Arc::new)
        .map_err(|e| e.to_string())
}

/// Guarda o token do bot no broker e devolve o id do segredo. Reiniciar com o mesmo token
/// reaproveita o envelope; token novo rotaciona o mesmo segredo em vez de empilhar outro.
pub fn guardar_token(broker: &SecretBroker, token: SecretValue) -> Result<Uuid, String> {
    use sha2::{Digest, Sha256};
    let hash: String = Sha256::digest(token.expose().as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    let existente = broker
        .descriptors()
        .map_err(|e| e.to_string())?
        .into_iter()
        .find(|d| d.name == NOME_SEGREDO && d.namespace == "canais" && d.revoked_at.is_none());
    match existente {
        Some(d) if d.sha256 == hash => Ok(d.uuid),
        Some(d) => broker
            .rotate(d.uuid, token)
            .map(|d| d.uuid)
            .map_err(|e| e.to_string()),
        None => broker
            .store(
                NOME_SEGREDO,
                "canais",
                ESCOPOS.iter().map(|s| s.to_string()).collect(),
                token,
            )
            .map(|d| d.uuid)
            .map_err(|e| e.to_string()),
    }
}

/// Liga o canal de producao: guarda o token no broker da pasta, monta o provedor da origem
/// oficial e confere o token com `getMe` antes de ouvir -- token errado para aqui, dizendo
/// o HTTP, e nao vira um laco de erro a cada 5 s.
pub async fn ligar_telegram(
    pasta: &Path,
    token: SecretValue,
    permitidos: BTreeSet<i64>,
    log: Registro,
) -> Result<CanalTelegram, String> {
    use phxclaw_channel_gateway::ChannelProviderV2;
    if permitidos.is_empty() {
        return Err("lista de chats vazia: ninguem poderia falar com o agente".into());
    }
    let broker = broker_em(pasta)?;
    let id = guardar_token(&broker, token)?;
    let (provider, sonda) = tokio::task::spawn_blocking(move || {
        let p = TelegramProvider::new(broker, id, CANAL).map_err(|e| e.to_string())?;
        let sonda = p.probe()?;
        Ok::<_, String>((p, sonda))
    })
    .await
    .map_err(|e| e.to_string())??;
    if !sonda.connected {
        return Err(format!(
            "telegram: getMe recusou o token ({})",
            sonda.error.unwrap_or_else(|| "sem detalhe".into())
        ));
    }
    let conta = sonda.account_id.unwrap_or_else(|| CANAL.into());
    (log)(&format!(
        "telegram: conectado como {} ({conta})",
        sonda.display_name.as_deref().unwrap_or("bot")
    ));
    CanalTelegram::novo(provider, conta, permitidos, pasta, log)
}

/// Lista de chats de `PHXCLAW_TELEGRAM_CHATS` ("123,-100456"). Id que nao e numero e erro,
/// e nao omissao: um chat digitado errado sumir calado da lista parece bloqueio sem motivo.
pub fn chats_da_lista(texto: &str) -> Result<BTreeSet<i64>, String> {
    texto
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| {
            s.parse::<i64>()
                .map_err(|_| format!("id de chat invalido em PHXCLAW_TELEGRAM_CHATS: {s}"))
        })
        .collect()
}

#[derive(Clone)]
pub struct CanalTelegram {
    provider: Arc<TelegramProvider>,
    conta: String,
    permitidos: Arc<BTreeSet<i64>>,
    principais: Arc<BTreeMap<i64, Uuid>>,
    gateway: ChannelGateway,
    offset_arq: PathBuf,
    log: Registro,
}

impl CanalTelegram {
    /// `pasta` guarda o offset e a evidencia do gateway. Cada chat permitido vira uma
    /// identidade ativa no gateway; o resto nao tem identidade e nao entra.
    pub fn novo(
        provider: TelegramProvider,
        conta: impl Into<String>,
        permitidos: BTreeSet<i64>,
        pasta: &Path,
        log: Registro,
    ) -> Result<Self, String> {
        std::fs::create_dir_all(pasta).map_err(|e| e.to_string())?;
        let conta = conta.into();
        let ledger = EvidenceLedger::open(pasta.join("canal-telegram.evidence.jsonl"))
            .map_err(|e| e.to_string())?;
        let gateway = ChannelGateway::new(LiveEventHub::new(16, 64), ledger);
        let mut principais = BTreeMap::new();
        for chat in &permitidos {
            let principal = phxclaw_types::new_uuid_v7();
            gateway
                .bind_identity(
                    CANAL,
                    &conta,
                    &chat.to_string(),
                    principal,
                    IdentityState::Active,
                )
                .map_err(|e| e.to_string())?;
            principais.insert(*chat, principal);
        }
        Ok(Self {
            provider: Arc::new(provider),
            conta,
            permitidos: Arc::new(permitidos),
            principais: Arc::new(principais),
            gateway,
            offset_arq: pasta.join("telegram.offset"),
            log,
        })
    }

    /// A unica decisao de quem pode falar com o agente e para quem ele pode escrever.
    pub fn permitido(&self, chat: i64) -> bool {
        self.permitidos.contains(&chat)
    }

    /// Proximo `update_id` esperado, do disco. Sem arquivo: o Telegram decide (os pendentes).
    pub fn offset(&self) -> Option<i64> {
        std::fs::read_to_string(&self.offset_arq)
            .ok()
            .and_then(|s| s.trim().parse().ok())
    }

    fn gravar_offset(&self, proximo: i64) -> Result<(), String> {
        let tmp = self.offset_arq.with_extension("offset.tmp");
        std::fs::write(&tmp, proximo.to_string())
            .and_then(|()| std::fs::rename(&tmp, &self.offset_arq))
            .map_err(|e| format!("telegram: offset nao gravou: {e}"))
    }

    /// Manda `texto` ao chat, partido em pedacos que cabem numa mensagem. E o unico caminho
    /// de saida: a resposta da tarefa e a ferramenta `channel_send` passam por aqui.
    pub async fn enviar(
        &self,
        chat: i64,
        texto: &str,
        sessao: Option<Uuid>,
    ) -> Result<usize, String> {
        if !self.permitido(chat) {
            return Err(format!("chat {chat} fora da lista de permitidos"));
        }
        let principal = self.principais.get(&chat).copied().unwrap_or_default();
        let pedacos = partir(texto, TELEGRAM_MAX_TEXT);
        let n = pedacos.len();
        for p in pedacos {
            let msg = OutboundMessage {
                uuid: phxclaw_types::new_uuid_v7(),
                session_uuid: sessao.unwrap_or_default(),
                principal_uuid: principal,
                channel: CANAL.into(),
                account_id: self.conta.clone(),
                conversation_id: chat.to_string(),
                text: p,
                created_at: Utc::now(),
            };
            let gw = self.gateway.clone();
            let pv = self.provider.clone();
            tokio::task::spawn_blocking(move || gw.send_with(&msg, pv.as_ref()))
                .await
                .map_err(|e| e.to_string())?
                .map_err(|e| e.to_string())?;
        }
        Ok(n)
    }

    /// Uma volta do long polling: busca, aceita ou ignora cada atualizacao, cria as tarefas
    /// e grava o offset. Devolve as respostas em andamento (o teste espera por elas; o laco
    /// as solta).
    pub async fn rodada(
        &self,
        s: &ApiState,
        espera_seg: u64,
    ) -> Result<Vec<tokio::task::JoinHandle<()>>, String> {
        let offset = self.offset();
        let pv = self.provider.clone();
        let updates = tokio::task::spawn_blocking(move || pv.get_updates(offset, espera_seg))
            .await
            .map_err(|e| e.to_string())?
            .map_err(|e| format!("telegram: getUpdates: {e}"))?;
        let mut respostas = Vec::new();
        for u in updates {
            if let Some(h) = self.tratar(s, &u).await {
                respostas.push(h);
            }
            // Depois de a tarefa estar gravada, nunca antes (ver o topo do modulo).
            self.gravar_offset(u.update_id + 1)?;
        }
        Ok(respostas)
    }

    async fn tratar(
        &self,
        s: &ApiState,
        u: &TelegramUpdate,
    ) -> Option<tokio::task::JoinHandle<()>> {
        let m = u.message.as_ref()?;
        let chat = m.chat.id;
        if !self.permitido(chat) {
            (self.log)(&format!(
                "telegram: mensagem do chat {chat} ignorada (fora da lista)"
            ));
            return None;
        }
        let Some(texto) = m.text.as_deref().filter(|t| !t.trim().is_empty()) else {
            let _ = self
                .enviar(chat, "So entendo mensagens de texto por enquanto.", None)
                .await;
            return None;
        };
        let entrada = InboundMessage::new(
            CANAL,
            &self.conta,
            chat.to_string(),
            chat.to_string(),
            m.message_id.to_string(),
            texto,
        );
        let sessao = match self.gateway.accept_inbound(entrada) {
            Ok(r) => r.session_uuid,
            Err(ChannelGatewayError::DuplicateMessage) => return None,
            Err(e) => {
                (self.log)(&format!("telegram: chat {chat} recusado pelo gateway: {e}"));
                return None;
            }
        };
        let pedido = NovaTarefa {
            objective: texto.to_string(),
            ..NovaTarefa::default()
        };
        match criar_tarefa(s, pedido) {
            Ok(c) => {
                (self.log)(&format!("telegram: chat {chat} -> tarefa {}", c.id));
                let canal = self.clone();
                Some(tokio::spawn(async move {
                    let texto = match c.fim.await {
                        Ok(t) => resposta_da_tarefa(&t),
                        Err(e) => format!("Tarefa {} interrompida: {e}", c.id),
                    };
                    if let Err(e) = canal.enviar(chat, &texto, Some(sessao)).await {
                        (canal.log)(&format!("telegram: resposta ao chat {chat} falhou: {e}"));
                    }
                }))
            }
            Err(r) => {
                let aviso = match r.retry_after {
                    Some(seg) => format!("Limite de tarefas atingido; tente em {seg} s."),
                    None => format!("Tarefa recusada: {}", r.erro),
                };
                let _ = self.enviar(chat, &aviso, Some(sessao)).await;
                None
            }
        }
    }

    /// O laco do processo: nunca termina; erro de rede espera e tenta de novo.
    pub async fn laco(self, s: ApiState) {
        (self.log)(&format!(
            "telegram: ouvindo {} chat(s) permitido(s)",
            self.permitidos.len()
        ));
        loop {
            match self
                .rodada(&s, phxclaw_channel_providers::TELEGRAM_MAX_POLL_SECS)
                .await
            {
                Ok(_respostas) => {}
                Err(e) => {
                    (self.log)(&e);
                    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                }
            }
        }
    }
}

/// O texto que volta ao chat quando a tarefa termina.
pub fn resposta_da_tarefa(t: &Task) -> String {
    match t.status {
        TaskStatus::Completed => t
            .answer
            .clone()
            .filter(|a| !a.trim().is_empty())
            .unwrap_or_else(|| format!("Tarefa {} concluida, sem texto de resposta.", t.id)),
        TaskStatus::AwaitingApproval => format!("Tarefa {} esperando aprovacao do plano.", t.id),
        outro => format!(
            "Tarefa {} terminou em {:?}: {}",
            t.id,
            outro,
            t.error.as_deref().unwrap_or("sem detalhe")
        ),
    }
}

/// Parte o texto em pedacos de ate `max` unidades UTF-16 (a conta do Telegram: um emoji
/// fora do plano basico vale 2). Corta numa quebra de linha quando ha uma na metade final
/// do pedaco, para nao partir paragrafo; nunca corta caractere ao meio.
pub fn partir(texto: &str, max: usize) -> Vec<String> {
    let max = max.max(2);
    let mut pedacos = Vec::new();
    let mut atual = String::new();
    let mut unidades = 0usize;
    for c in texto.chars() {
        let u = c.len_utf16();
        if unidades + u > max {
            let corte = atual
                .rfind('\n')
                .filter(|&i| atual[..i].encode_utf16().count() >= max / 2);
            match corte {
                Some(i) => {
                    let resto = atual[i + 1..].to_string();
                    atual.truncate(i + 1);
                    pedacos.push(std::mem::replace(&mut atual, resto));
                }
                None => pedacos.push(std::mem::take(&mut atual)),
            }
            unidades = atual.encode_utf16().count();
        }
        atual.push(c);
        unidades += u;
    }
    if !atual.is_empty() || pedacos.is_empty() {
        pedacos.push(atual);
    }
    pedacos
}

/// O agente escreve para um chat da lista durante a tarefa. Capacidade `channel.send`,
/// fora do padrao: falar em nome do dono num chat e efeito externo que o operador concede.
pub struct ChannelSendTool {
    pub canal: CanalTelegram,
}

impl Tool for ChannelSendTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "channel_send".into(),
            description: "Send a text message to an allowed Telegram chat (by chat id). \
Long text is split automatically."
                .into(),
            parameters: json!({"type":"object","properties":{
                "to":{"type":["string","integer"],"description":"chat id from the allowed list"},
                "text":{"type":"string"}},"required":["to","text"]}),
        }
    }

    fn capability(&self) -> &'static str {
        "channel.send"
    }

    fn run<'a>(
        &'a self,
        args: Value,
        _ctx: &'a ToolContext,
    ) -> BoxFut<'a, Result<ToolOutput, ToolError>> {
        Box::pin(async move {
            let chat = match &args["to"] {
                Value::Number(n) => n.as_i64(),
                Value::String(s) => s.trim().parse().ok(),
                _ => None,
            }
            .ok_or_else(|| ToolError::InvalidArguments("to: id numerico do chat".into()))?;
            let texto = args["text"]
                .as_str()
                .filter(|t| !t.trim().is_empty())
                .ok_or_else(|| ToolError::InvalidArguments("text vazio".into()))?;
            if !self.canal.permitido(chat) {
                return Err(ToolError::Denied(format!(
                    "chat {chat} fora da lista de permitidos"
                )));
            }
            let n = self
                .canal
                .enviar(chat, texto, None)
                .await
                .map_err(ToolError::Failed)?;
            Ok(ToolOutput::text(format!(
                "mensagem enviada ao chat {chat} em {n} parte(s)"
            )))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn partir_respeita_o_teto_e_nao_perde_nada() {
        let t = "a".repeat(10_000);
        let p = partir(&t, 4096);
        assert_eq!(p.len(), 3);
        assert!(p.iter().all(|x| x.encode_utf16().count() <= 4096));
        assert_eq!(p.concat(), t);
        assert_eq!(partir("", 4096), vec![String::new()]);
        assert_eq!(partir("curto", 4096), vec!["curto".to_string()]);
    }

    #[test]
    fn partir_conta_utf16_e_prefere_quebra_de_linha() {
        // Emoji vale 2 unidades: 3000 deles sao 6000 unidades, dois pedacos.
        let t = "\u{1F600}".repeat(3000);
        let p = partir(&t, 4096);
        assert_eq!(p.len(), 2);
        assert!(p.iter().all(|x| x.encode_utf16().count() <= 4096));
        assert_eq!(p.concat(), t);
        let t = format!("{}\n{}", "x".repeat(3000), "y".repeat(3000));
        let p = partir(&t, 4096);
        assert_eq!(p[0], format!("{}\n", "x".repeat(3000)));
        assert_eq!(p.concat(), t);
    }

    #[test]
    fn lista_de_chats_recusa_id_que_nao_e_numero() {
        assert_eq!(
            chats_da_lista(" 1, -100200 ,,").unwrap(),
            [1, -100200].into_iter().collect()
        );
        assert!(chats_da_lista("1,abc").is_err());
    }
}
