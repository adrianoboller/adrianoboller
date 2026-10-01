//! Canais de mensagem do agente: UM laco para todos, um provedor por servico.
//!
//! O desenho saiu do Telegram, que foi o primeiro canal de verdade, e as decisoes dele
//! valem para todos os outros porque moram aqui e nao em cada provedor:
//! - a lista de conversas permitidas e UMA decisao (`Canal::permitido`), usada pela entrada
//!   e pela ferramenta `channel_send`; conversa fora dela nem chega ao gateway;
//! - a tarefa nasce por `api::criar_tarefa`, a mesma funcao da rota HTTP: validacao, modelo
//!   padrao e balde de fichas sao os mesmos em todas as portas;
//! - o cursor grava DEPOIS de a tarefa estar gravada: cair entre as duas coisas reprocessa
//!   uma mensagem (tarefa duplicada) em vez de perder uma (pedido sem resposta);
//! - a resposta sai partida no limite do canal, medido na unidade que o servico conta
//!   (UTF-16 no Telegram, byte no IRC e no Matrix, caractere no resto);
//! - segredo mora so no SecretBroker e e lido por concessao curta a cada chamada
//!   (`Credencial::com`), que tambem tira o segredo de todo erro que volta.
//!
//! O provedor so sabe falar com o servico dele: receber um lote desde um cursor e enviar
//! UM pedaco de texto. Servico que empurra (webhook) ou que entrega uma vez so (Signal,
//! IRC) grava primeiro numa `caixa::Caixa` em disco, e o cursor passa a ser o da caixa --
//! assim a regra do cursor vale igual para quem pergunta e para quem e avisado.

pub mod bip340;
pub mod caixa;
pub mod cripto;
pub mod discord;
pub mod email;
pub mod feishu;
pub mod googlechat;
pub mod http;
pub mod irc;
pub mod ligar;
pub mod line;
pub mod mastodon;
pub mod matrix;
pub mod mattermost;
pub mod meta;
pub mod nostr;
pub mod reddit;
pub mod rocketchat;
pub mod signal;
pub mod slack;
pub mod sms;
pub mod teams;
pub mod tls;
pub mod viber;
pub mod webchat;
pub mod webhook;
pub mod xmpp;
pub mod zulip;

use crate::api::{ApiState, NovaTarefa, criar_tarefa};
use crate::tarefa::{Task, TaskStatus};
use chrono::Utc;
use phxclaw_agent_core::{BoxFut, Tool, ToolContext, ToolError, ToolOutput, ToolSpec};
use phxclaw_channel_gateway::{
    ChannelGateway, ChannelGatewayError, ChannelProvider, IdentityState, InboundMessage,
    OutboundMessage, ProviderReceipt,
};
use phxclaw_evidence_ledger::EvidenceLedger;
use phxclaw_live_bus::LiveEventHub;
use phxclaw_secret_broker::{FileMasterKeyProvider, SecretBroker, SecretValue};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use uuid::Uuid;

pub use http::Credencial;

/// Para onde vao as linhas de registro do canal (terminal no binario, vetor no teste).
pub type Registro = Arc<dyn Fn(&str) + Send + Sync>;

/// Em que unidade o servico conta o teto de uma mensagem. Contar errado nao e detalhe:
/// o Telegram recusa a mensagem inteira se o emoji fora do plano basico, que vale 2, passa.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unidade {
    Utf16,
    Caractere,
    Byte,
}

impl Unidade {
    fn de(self, c: char) -> usize {
        match self {
            Unidade::Utf16 => c.len_utf16(),
            Unidade::Caractere => 1,
            Unidade::Byte => c.len_utf8(),
        }
    }

    pub fn medir(self, s: &str) -> usize {
        s.chars().map(|c| self.de(c)).sum()
    }
}

/// Uma mensagem que chegou. `conversa` e o que a lista de permitidos confere e para onde a
/// resposta volta; `id` e o que o gateway usa para nao tratar a mesma mensagem duas vezes.
/// `texto` vazio e foto, audio, figurinha: a mensagem existe, mas o agente nao a entende.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Mensagem {
    pub conversa: String,
    pub autor: String,
    pub id: String,
    pub texto: Option<String>,
}

/// Um item do lote. `cursor` e o que se grava DEPOIS de tratar este item; item sem
/// mensagem (edicao, evento de sistema, mensagem do proprio bot) so faz o cursor andar.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Entrada {
    pub cursor: Option<String>,
    pub mensagem: Option<Mensagem>,
}

/// O que cada servico precisa saber fazer. Os dois metodos sao bloqueantes (HTTP
/// bloqueante, soquete) e o laco os chama em `spawn_blocking`.
pub trait Provedor: Send + Sync + 'static {
    /// Nome do canal ("telegram", "discord"...): vai no registro, no gateway e no arquivo
    /// do cursor.
    fn nome(&self) -> &str;
    /// Teto de UM pedaco de texto e a unidade em que o servico o conta.
    fn limite(&self) -> (usize, Unidade);
    /// Quanto o laco espera numa volta sem nada; abaixo do prazo do cliente HTTP.
    fn espera_maxima(&self) -> u64 {
        25
    }
    /// O lote desde `cursor` (None: o servico decide, ou "a partir de agora").
    fn receber(&self, cursor: Option<&str>, espera_seg: u64) -> Result<Vec<Entrada>, String>;
    /// Manda UM pedaco que ja cabe no limite; devolve o id que o servico deu.
    fn enviar(&self, conversa: &str, texto: &str) -> Result<String, String>;
}

/// O gateway registra a evidencia de envio e confere o canal; o provedor so manda.
struct Ponte<'a>(&'a dyn Provedor);

impl ChannelProvider for Ponte<'_> {
    fn channel(&self) -> &str {
        self.0.nome()
    }

    fn send(&self, m: &OutboundMessage) -> Result<ProviderReceipt, String> {
        let id = self.0.enviar(&m.conversation_id, &m.text)?;
        Ok(ProviderReceipt {
            provider_message_id: id,
            metadata: json!({"provider": self.0.nome(), "account_id": m.account_id}),
        })
    }
}

/// Manda UM pedaco por um provedor do `phxclaw-channel-providers` (Telegram, Discord,
/// Slack, WhatsApp ja sabiam enviar la; aqui so se aprende a receber).
pub fn enviar_pelo(p: &dyn ChannelProvider, conversa: &str, texto: &str) -> Result<String, String> {
    let msg = OutboundMessage {
        uuid: phxclaw_types::new_uuid_v7(),
        session_uuid: Uuid::nil(),
        principal_uuid: Uuid::nil(),
        channel: p.channel().to_string(),
        account_id: String::new(),
        conversation_id: conversa.to_string(),
        text: texto.to_string(),
        created_at: Utc::now(),
    };
    p.send(&msg).map(|r| r.provider_message_id)
}

/// Provedor do `phxclaw-channel-providers` criado na primeira chamada: o construtor dele
/// cria o cliente HTTP bloqueante, que nao pode nascer dentro do runtime assincrono.
pub struct Preguicoso<P> {
    feito: std::sync::OnceLock<P>,
    fazer: Box<dyn Fn() -> Result<P, String> + Send + Sync>,
}

impl<P: Send + Sync> Preguicoso<P> {
    pub fn novo(fazer: impl Fn() -> Result<P, String> + Send + Sync + 'static) -> Self {
        Self {
            feito: std::sync::OnceLock::new(),
            fazer: Box::new(fazer),
        }
    }

    pub fn obter(&self) -> Result<&P, String> {
        if let Some(p) = self.feito.get() {
            return Ok(p);
        }
        let p = (self.fazer)()?;
        Ok(self.feito.get_or_init(|| p))
    }
}

/// Quem pergunta ao servico sem espera longa (Discord, Slack, Mattermost...) dorme um
/// pouco quando nao veio nada: sem isso o laco viraria uma rajada de pedidos e o servico
/// responderia com 429.
pub fn pausa_se_vazio(lote: &[Entrada], espera_seg: u64) {
    if lote.iter().all(|e| e.mensagem.is_none()) && espera_seg > 0 {
        std::thread::sleep(std::time::Duration::from_secs(espera_seg.min(3)));
    }
}

/// Broker de segredos da pasta do agente; a chave mestra nasce 0600 na primeira vez.
///
/// UM broker por pasta no processo: dois `SecretBroker` vivos sobre o mesmo
/// `evidence.jsonl` encadeiam cada um a partir do seu ultimo registro, e a corrente se
/// parte -- medido com o `speak`, o `transcribe` e o `voice_list` pedindo a mesma chave da
/// ElevenLabs: a abertura seguinte recusou o livro («invalid record at line 4»). A forja, o
/// MCP e os canais abrem a pasta deles a cada montagem de agente, e o servidor monta um
/// agente por tarefa: pelo mesmo mecanismo (nao medido ali), duas tarefas ao mesmo tempo
/// seriam dois brokers.
pub fn broker_em(pasta: &Path) -> Result<Arc<SecretBroker>, String> {
    static ABERTOS: std::sync::OnceLock<
        std::sync::Mutex<std::collections::HashMap<PathBuf, Arc<SecretBroker>>>,
    > = std::sync::OnceLock::new();
    let pasta = std::path::absolute(pasta).map_err(|e| e.to_string())?;
    let mut abertos = ABERTOS
        .get_or_init(Default::default)
        .lock()
        .map_err(|e| e.to_string())?;
    if let Some(b) = abertos.get(&pasta) {
        return Ok(b.clone());
    }
    let b = abrir_broker(&pasta)?;
    abertos.insert(pasta, b.clone());
    Ok(b)
}

fn abrir_broker(pasta: &Path) -> Result<Arc<SecretBroker>, String> {
    let dir = pasta.join("segredos");
    let chave = Arc::new(FileMasterKeyProvider::new(dir.join("master.key")));
    chave.ensure().map_err(|e| e.to_string())?;
    let ledger = EvidenceLedger::open(dir.join("evidence.jsonl")).map_err(|e| e.to_string())?;
    SecretBroker::new(dir.join("cofre"), chave, LiveEventHub::new(16, 16), ledger)
        .map(Arc::new)
        .map_err(|e| e.to_string())
}

/// Os usos que um segredo de canal concede. Uma lista so, para a concessao de
/// `Credencial::com` e o envelope gravado aqui nunca divergirem.
pub fn escopos(canal: &str) -> Vec<String> {
    ["send", "probe", "receive", "verify"]
        .iter()
        .map(|u| format!("channel:{canal}:{u}"))
        .collect()
}

/// A regra de guardar um segredo, UMA so para os canais e para quem mais guardar token
/// (as forjas tambem chamam por `canal::guardar_segredo`): o mesmo valor reaproveita o
/// envelope; valor novo rotaciona o mesmo segredo em vez de empilhar outro.
pub fn guardar_segredo(
    broker: &SecretBroker,
    nome: &str,
    namespace: &str,
    escopos: &[&str],
    valor: SecretValue,
) -> Result<Uuid, String> {
    use sha2::{Digest, Sha256};
    let hash: String = Sha256::digest(valor.expose().as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    match segredo_guardado(broker, nome, namespace)? {
        Some(d) if d.sha256 == hash => Ok(d.uuid),
        Some(d) => broker
            .rotate(d.uuid, valor)
            .map(|d| d.uuid)
            .map_err(|e| e.to_string()),
        None => broker
            .store(
                nome,
                namespace,
                escopos.iter().map(|s| s.to_string()).collect(),
                valor,
            )
            .map(|d| d.uuid)
            .map_err(|e| e.to_string()),
    }
}

/// Guarda um segredo de canal com os escopos de `escopos(canal)`.
pub fn guardar_do_canal(
    broker: &SecretBroker,
    nome: &str,
    canal: &str,
    valor: SecretValue,
) -> Result<Uuid, String> {
    let e = escopos(canal);
    let refs: Vec<&str> = e.iter().map(String::as_str).collect();
    guardar_segredo(broker, nome, "canais", &refs, valor)
}

/// O segredo vivo com esse nome, se ha. A mesma pergunta serve a quem guarda e a quem
/// liga um canal sem o valor no ambiente -- o envelope de antes vale, e o segredo nao
/// precisa morar no ambiente do processo.
pub fn segredo_guardado(
    broker: &SecretBroker,
    nome: &str,
    namespace: &str,
) -> Result<Option<phxclaw_secret_broker::SecretDescriptor>, String> {
    Ok(broker
        .descriptors()
        .map_err(|e| e.to_string())?
        .into_iter()
        .find(|d| d.name == nome && d.namespace == namespace && d.revoked_at.is_none()))
}

/// O canal ligado: um provedor, a lista de conversas e o cursor em disco. O laco, a
/// decisao de quem fala e a saida partida sao estes, para todo provedor.
#[derive(Clone)]
pub struct Canal {
    provedor: Arc<dyn Provedor>,
    conta: String,
    permitidos: Arc<BTreeSet<String>>,
    principais: Arc<BTreeMap<String, Uuid>>,
    /// Conversa -> tarefa parada em `AwaitingInput` que perguntou nela. A proxima mensagem
    /// dessa conversa e a resposta, nao um pedido novo.
    esperas: Arc<std::sync::Mutex<BTreeMap<String, String>>>,
    gateway: ChannelGateway,
    cursor_arq: PathBuf,
    log: Registro,
}

impl Canal {
    /// `pasta` guarda o cursor e a evidencia do gateway. Cada conversa permitida vira uma
    /// identidade ativa no gateway; o resto nao tem identidade e nao entra.
    pub fn novo<P: Provedor, T: ToString>(
        provedor: P,
        conta: impl Into<String>,
        permitidos: BTreeSet<T>,
        pasta: &Path,
        log: Registro,
    ) -> Result<Self, String> {
        Self::de_arc(Arc::new(provedor), conta, permitidos, pasta, log)
    }

    /// Como `novo`, para o provedor que tambem e recebedor de webhook e e compartilhado
    /// com a rota HTTP.
    pub fn de_arc<T: ToString>(
        provedor: Arc<dyn Provedor>,
        conta: impl Into<String>,
        permitidos: BTreeSet<T>,
        pasta: &Path,
        log: Registro,
    ) -> Result<Self, String> {
        std::fs::create_dir_all(pasta).map_err(|e| e.to_string())?;
        let nome = provedor.nome().to_string();
        let conta = conta.into();
        let ledger = EvidenceLedger::open(pasta.join(format!("canal-{nome}.evidence.jsonl")))
            .map_err(|e| e.to_string())?;
        let gateway = ChannelGateway::new(LiveEventHub::new(16, 64), ledger);
        let permitidos: BTreeSet<String> = permitidos
            .into_iter()
            .map(|p| p.to_string().trim().to_string())
            .filter(|p| !p.is_empty())
            .collect();
        let mut principais = BTreeMap::new();
        for conversa in &permitidos {
            let principal = phxclaw_types::new_uuid_v7();
            gateway
                .bind_identity(&nome, &conta, conversa, principal, IdentityState::Active)
                .map_err(|e| e.to_string())?;
            principais.insert(conversa.clone(), principal);
        }
        Ok(Self {
            cursor_arq: pasta.join(format!("{nome}.offset")),
            provedor,
            conta,
            permitidos: Arc::new(permitidos),
            principais: Arc::new(principais),
            esperas: Arc::default(),
            gateway,
            log,
        })
    }

    pub fn nome(&self) -> &str {
        self.provedor.nome()
    }

    /// A unica decisao de quem pode falar com o agente e para quem ele pode escrever.
    pub fn permitido(&self, conversa: &str) -> bool {
        self.permitidos.contains(conversa.trim())
    }

    /// Cursor gravado em disco. Sem arquivo: o provedor decide (os pendentes, ou "agora").
    pub fn cursor(&self) -> Option<String> {
        std::fs::read_to_string(&self.cursor_arq)
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    }

    /// O cursor lido como numero, o formato do Telegram (o proximo `update_id`).
    pub fn offset(&self) -> Option<i64> {
        self.cursor().and_then(|s| s.parse().ok())
    }

    fn gravar_cursor(&self, cursor: &str) -> Result<(), String> {
        let tmp = self.cursor_arq.with_extension("offset.tmp");
        std::fs::write(&tmp, cursor)
            .and_then(|()| std::fs::rename(&tmp, &self.cursor_arq))
            .map_err(|e| format!("{}: cursor nao gravou: {e}", self.nome()))
    }

    /// Manda `texto` a conversa, partido em pedacos que cabem numa mensagem. E o unico
    /// caminho de saida: a resposta da tarefa e a ferramenta `channel_send` passam por aqui.
    pub async fn enviar(
        &self,
        conversa: &str,
        texto: &str,
        sessao: Option<Uuid>,
    ) -> Result<usize, String> {
        if !self.permitido(conversa) {
            return Err(format!("conversa {conversa} fora da lista de permitidos"));
        }
        let principal = self.principais.get(conversa).copied().unwrap_or_default();
        let (max, unidade) = self.provedor.limite();
        let pedacos = partir_em(texto, max, unidade);
        let n = pedacos.len();
        for p in pedacos {
            let msg = OutboundMessage {
                uuid: phxclaw_types::new_uuid_v7(),
                session_uuid: sessao.unwrap_or_default(),
                principal_uuid: principal,
                channel: self.nome().to_string(),
                account_id: self.conta.clone(),
                conversation_id: conversa.to_string(),
                text: p,
                created_at: Utc::now(),
            };
            let gw = self.gateway.clone();
            let pv = self.provedor.clone();
            tokio::task::spawn_blocking(move || gw.send_with(&msg, &Ponte(pv.as_ref())))
                .await
                .map_err(|e| e.to_string())?
                .map_err(|e| e.to_string())?;
        }
        Ok(n)
    }

    /// Uma volta: busca o lote, aceita ou ignora cada mensagem, cria as tarefas e grava o
    /// cursor. Devolve as respostas em andamento (o teste espera por elas; o laco as solta).
    pub async fn rodada(
        &self,
        s: &ApiState,
        espera_seg: u64,
    ) -> Result<Vec<tokio::task::JoinHandle<()>>, String> {
        let cursor = self.cursor();
        let pv = self.provedor.clone();
        let nome = self.nome().to_string();
        let lote = tokio::task::spawn_blocking(move || pv.receber(cursor.as_deref(), espera_seg))
            .await
            .map_err(|e| e.to_string())?
            .map_err(|e| format!("{nome}: receber: {e}"))?;
        let mut respostas = Vec::new();
        for e in lote {
            if let Some(m) = &e.mensagem
                && let Some(h) = self.tratar(s, m).await
            {
                respostas.push(h);
            }
            // Depois de a tarefa estar gravada, nunca antes (ver o topo do modulo).
            if let Some(c) = &e.cursor {
                self.gravar_cursor(c)?;
            }
        }
        Ok(respostas)
    }

    async fn tratar(&self, s: &ApiState, m: &Mensagem) -> Option<tokio::task::JoinHandle<()>> {
        let nome = self.nome();
        let conversa = m.conversa.as_str();
        if !self.permitido(conversa) {
            // So o id: o texto de quem nao foi autorizado nao entra no registro.
            (self.log)(&format!(
                "{nome}: mensagem da conversa {conversa} ignorada (fora da lista)"
            ));
            return None;
        }
        let Some(texto) = m.texto.as_deref().filter(|t| !t.trim().is_empty()) else {
            let _ = self
                .enviar(
                    conversa,
                    "So entendo mensagens de texto por enquanto.",
                    None,
                )
                .await;
            return None;
        };
        let entrada =
            InboundMessage::new(nome, &self.conta, conversa, conversa, m.id.as_str(), texto);
        let sessao = match self.gateway.accept_inbound(entrada) {
            Ok(r) => r.session_uuid,
            Err(ChannelGatewayError::DuplicateMessage) => return None,
            Err(e) => {
                (self.log)(&format!(
                    "{nome}: conversa {conversa} recusada pelo gateway: {e}"
                ));
                return None;
            }
        };
        // A tarefa desta conversa perguntou algo (`ask_user`, regra `perguntar`): a mensagem
        // e a resposta, pela mesma `perguntas::responder` da rota HTTP e da CLI.
        let esperando = self
            .esperas
            .lock()
            .ok()
            .and_then(|mut g| g.remove(conversa));
        if let Some(id) = esperando {
            match crate::perguntas::responder(&id, texto) {
                Ok(()) => {
                    (self.log)(&format!(
                        "{nome}: conversa {conversa} respondeu a tarefa {id}"
                    ));
                    return None;
                }
                // Ninguem espera mais (prazo venceu no mesmo instante): vira pedido novo,
                // que e o que a pessoa ve acontecer -- melhor que sumir calada.
                Err(e) => (self.log)(&format!(
                    "{nome}: tarefa {id} nao espera mais resposta ({e}); mensagem vira tarefa"
                )),
            }
        }
        let pedido = NovaTarefa {
            objective: texto.to_string(),
            ..NovaTarefa::default()
        };
        match criar_tarefa(s, pedido) {
            Ok(c) => {
                (self.log)(&format!("{nome}: conversa {conversa} -> tarefa {}", c.id));
                let canal = self.clone();
                let conversa = conversa.to_string();
                let store = s.store.clone();
                Some(tokio::spawn(async move {
                    let texto = match canal
                        .acompanhar(&store, &conversa, &c.id, c.fim, sessao)
                        .await
                    {
                        Ok(t) => resposta_da_tarefa(&t),
                        Err(e) => format!("Tarefa {} interrompida: {e}", c.id),
                    };
                    if let Err(e) = canal.enviar(&conversa, &texto, Some(sessao)).await {
                        (canal.log)(&format!(
                            "{}: resposta a conversa {conversa} falhou: {e}",
                            canal.nome()
                        ));
                    }
                }))
            }
            Err(r) => {
                let aviso = match r.retry_after {
                    Some(seg) => format!("Limite de tarefas atingido; tente em {seg} s."),
                    None => format!("Tarefa recusada: {}", r.erro),
                };
                let _ = self.enviar(conversa, &aviso, Some(sessao)).await;
                None
            }
        }
    }

    /// Espera a tarefa terminar e, enquanto isso, olha o estado gravado: quando ela para em
    /// `AwaitingInput`, a pergunta vai a conversa e a conversa fica marcada como esperando
    /// (a proxima mensagem dela e a resposta). O olhar e pelo `task.json` porque e ali que
    /// o motor publica a pergunta para a API e a tela; uma via so para todos.
    async fn acompanhar(
        &self,
        store: &crate::tarefa::TaskStore,
        conversa: &str,
        id: &str,
        mut fim: tokio::task::JoinHandle<Task>,
        sessao: Uuid,
    ) -> Result<Task, tokio::task::JoinError> {
        let mut perguntada: Option<String> = None;
        let r = loop {
            tokio::select! {
                r = &mut fim => break r,
                _ = tokio::time::sleep(std::time::Duration::from_millis(250)) => {}
            }
            let Ok(t) = store.load(id) else { continue };
            match (&t.status, t.question) {
                (TaskStatus::AwaitingInput, Some(q)) => {
                    // A mesma pergunta nao sai duas vezes; a marca de espera vai ANTES do
                    // envio, para a resposta rapida ja ter para onde ir.
                    let marca = format!("{}|{q}", t.updated_at.timestamp_micros());
                    if perguntada.as_deref() != Some(marca.as_str()) {
                        if let Ok(mut g) = self.esperas.lock() {
                            g.insert(conversa.to_string(), id.to_string());
                        }
                        if let Err(e) = self.enviar(conversa, &q, Some(sessao)).await {
                            (self.log)(&format!(
                                "{}: pergunta a conversa {conversa} falhou: {e}",
                                self.nome()
                            ));
                        }
                        perguntada = Some(marca);
                    }
                }
                _ => {
                    perguntada = None;
                    self.soltar_espera(conversa, id);
                }
            }
        };
        self.soltar_espera(conversa, id);
        r
    }

    /// Tira a marca de espera da conversa, se ainda e desta tarefa.
    fn soltar_espera(&self, conversa: &str, id: &str) {
        if let Ok(mut g) = self.esperas.lock()
            && g.get(conversa).is_some_and(|x| x == id)
        {
            g.remove(conversa);
        }
    }

    /// O laco do processo: nunca termina; erro de rede espera e tenta de novo.
    pub async fn laco(self, s: ApiState) {
        (self.log)(&format!(
            "{}: ouvindo {} conversa(s) permitida(s)",
            self.nome(),
            self.permitidos.len()
        ));
        let espera = self.provedor.espera_maxima();
        loop {
            match self.rodada(&s, espera).await {
                Ok(_respostas) => {}
                Err(e) => {
                    (self.log)(&e);
                    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                }
            }
        }
    }
}

/// O texto que volta a conversa quando a tarefa termina.
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

/// Parte em unidades UTF-16 (a conta do Telegram). Ver `partir_em`.
pub fn partir(texto: &str, max: usize) -> Vec<String> {
    partir_em(texto, max, Unidade::Utf16)
}

/// Parte o texto em pedacos de ate `max` unidades. Corta numa quebra de linha quando ha
/// uma na metade final do pedaco, para nao partir paragrafo; nunca corta caractere ao meio.
/// O piso de 4 e o maior caractere em qualquer unidade: abaixo dele um caractere nao cabe
/// em pedaco nenhum e o laco nunca andaria.
pub fn partir_em(texto: &str, max: usize, unidade: Unidade) -> Vec<String> {
    let max = max.max(4);
    let mut pedacos = Vec::new();
    let mut atual = String::new();
    let mut unidades = 0usize;
    for c in texto.chars() {
        let u = unidade.de(c);
        if unidades + u > max {
            let corte = atual
                .rfind('\n')
                .filter(|&i| unidade.medir(&atual[..i]) >= max / 2);
            match corte {
                Some(i) => {
                    let resto = atual[i + 1..].to_string();
                    atual.truncate(i + 1);
                    pedacos.push(std::mem::replace(&mut atual, resto));
                }
                None => pedacos.push(std::mem::take(&mut atual)),
            }
            unidades = unidade.medir(&atual);
        }
        atual.push(c);
        unidades += u;
    }
    if !atual.is_empty() || pedacos.is_empty() {
        pedacos.push(atual);
    }
    pedacos
}

/// O agente escreve para uma conversa da lista durante a tarefa. Capacidade
/// `channel.send`, fora do padrao: falar em nome do dono e efeito externo que o operador
/// concede. O parametro `channel` existe para o modelo dizer onde quer falar; canal que nao
/// esta ligado neste processo e recusa, nunca troca silenciosa de canal.
pub struct ChannelSendTool {
    pub canal: Canal,
}

impl Tool for ChannelSendTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "channel_send".into(),
            description: format!(
                "Send a text message to an allowed conversation of the connected messaging \
channel ({}). Long text is split automatically.",
                self.canal.nome()
            ),
            parameters: json!({"type":"object","properties":{
                "channel":{"type":"string","description":"channel name; defaults to the connected one"},
                "to":{"type":["string","integer"],"description":"conversation id from the allowed list"},
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
            if let Some(c) = args["channel"].as_str().map(str::trim)
                && !c.is_empty()
                && c != self.canal.nome()
            {
                return Err(ToolError::Denied(format!(
                    "canal {c} nao esta ligado; o ligado e {}",
                    self.canal.nome()
                )));
            }
            let conversa = match &args["to"] {
                Value::Number(n) => Some(n.to_string()),
                Value::String(s) if !s.trim().is_empty() => Some(s.trim().to_string()),
                _ => None,
            }
            .ok_or_else(|| ToolError::InvalidArguments("to: id da conversa".into()))?;
            let texto = args["text"]
                .as_str()
                .filter(|t| !t.trim().is_empty())
                .ok_or_else(|| ToolError::InvalidArguments("text vazio".into()))?;
            if !self.canal.permitido(&conversa) {
                return Err(ToolError::Denied(format!(
                    "conversa {conversa} fora da lista de permitidos"
                )));
            }
            let n = self
                .canal
                .enviar(&conversa, texto, None)
                .await
                .map_err(ToolError::Failed)?;
            Ok(ToolOutput::text(format!(
                "mensagem enviada a conversa {conversa} em {n} parte(s)"
            )))
        })
    }
}

/// Lista separada por virgula, sem vazios. Id repetido conta uma vez.
pub fn lista(texto: &str) -> BTreeSet<String> {
    texto
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect()
}

/// Cursor de quem ouve varias conversas: o ultimo id visto em cada uma, em JSON. Uma
/// conversa nova na lista comeca do "agora" dela, sem engolir o historico.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MapaCursor(pub BTreeMap<String, String>);

impl MapaCursor {
    pub fn ler(cursor: Option<&str>) -> Self {
        Self(
            cursor
                .and_then(|c| serde_json::from_str(c).ok())
                .unwrap_or_default(),
        )
    }

    pub fn texto(&self) -> String {
        serde_json::to_string(&self.0).unwrap_or_default()
    }
}

/// Conversas permitidas que viram parametro de consulta: o provedor de quem ouve canais
/// (Discord, Slack...) le so as da lista, nunca o servidor inteiro.
pub fn conversas_ouvidas(permitidos: &BTreeSet<String>) -> Vec<String> {
    permitidos.iter().cloned().collect()
}

/// Ordena ids numericos como numero e nao como texto ("10" depois de "9"). Id que nao e
/// numero fica no fim, na ordem do texto.
pub fn ordem_numerica(a: &str, b: &str) -> std::cmp::Ordering {
    match (a.parse::<u128>(), b.parse::<u128>()) {
        (Ok(x), Ok(y)) => x.cmp(&y),
        (Ok(_), Err(_)) => std::cmp::Ordering::Less,
        (Err(_), Ok(_)) => std::cmp::Ordering::Greater,
        _ => a.cmp(b),
    }
}

/// Tira as marcas de HTML de um texto (Mastodon e Matrix formatado entregam HTML); `<br>` e
/// `</p>` viram quebra de linha, e as entidades basicas voltam a ser caractere.
pub fn texto_de_html(html: &str) -> String {
    let mut saida = String::new();
    let mut tag = String::new();
    let mut dentro = false;
    for c in html.chars() {
        match (dentro, c) {
            (false, '<') => {
                dentro = true;
                tag.clear();
            }
            (true, '>') => {
                dentro = false;
                let t = tag.trim().to_ascii_lowercase();
                if t.starts_with("br") || t == "/p" {
                    saida.push('\n');
                }
            }
            (true, c) => tag.push(c),
            (false, c) => saida.push(c),
        }
    }
    saida
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
        .trim()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn partir_em_bytes_nao_corta_caractere_e_respeita_o_teto() {
        let t = "\u{e7}".repeat(300); // 2 bytes cada
        let p = partir_em(&t, 401, Unidade::Byte);
        assert!(
            p.iter().all(|x| x.len() <= 401),
            "{:?}",
            p.iter().map(|x| x.len()).collect::<Vec<_>>()
        );
        assert_eq!(p.concat(), t);
        assert_eq!(p.len(), 2);
        let p = partir_em("abcdef", 3, Unidade::Caractere);
        assert_eq!(p, vec!["abcd".to_string(), "ef".to_string()], "piso de 4");
    }

    #[test]
    fn partir_no_limite_exato_nao_parte() {
        let t = "a".repeat(2000);
        assert_eq!(partir_em(&t, 2000, Unidade::Caractere).len(), 1);
        assert_eq!(
            partir_em(&format!("{t}b"), 2000, Unidade::Caractere).len(),
            2
        );
    }

    #[test]
    fn html_vira_texto() {
        assert_eq!(
            texto_de_html("<p><span>@bot</span> oi &amp; tchau<br/>linha</p>"),
            "@bot oi & tchau\nlinha"
        );
    }

    #[test]
    fn ordem_numerica_nao_e_ordem_de_texto() {
        let mut v = vec!["10", "9", "100"];
        v.sort_by(|a, b| ordem_numerica(a, b));
        assert_eq!(v, vec!["9", "10", "100"]);
    }
}
