//! Terminal compartilhado do IDE (o «Live Share» do PhxClaw, decisao do dono de 09/10/2026):
//! um anfitriao, N convidados, um cursor -- o do anfitriao. Desenho e riscos em
//! `docs/propostas/live-share-terminal.md`.
//!
//! O que vale saber antes de mexer:
//! - **O convidado so CONSOME o que a sessao do anfitriao ja produz** (`ide::sessao`): a grade
//!   serializada uma vez vai ao anfitriao e a cada convidado. Nao ha segundo PTY, segundo
//!   Helix nem segunda serializacao (lei «funcao nao se duplica»).
//! - **Somente leitura por construcao, nao por filtro.** O convidado de leitura nao recebe
//!   canal nenhum ate o terminal: a tarefa dele nao tem por onde mandar tecla. O que escreve
//!   recebe o canal `entrada` -- e o anfitriao confere de novo, ao aplicar, que aquele
//!   convidado ainda existe e ainda escreve.
//! - **Escrever pede DUAS coisas**: o anfitriao conceder no convite (`escrita: true`) E o
//!   convidado provar pelo RBAC a mesma decisao da rota do terminal (`rbac::conferir_rota`
//!   com `GET /v1/ide/terminal`, que e do dono: o `:sh` do Helix e shell no hospedeiro).
//!   Faltou uma, entra so lendo e a resposta diz.
//! - **O token do convite**: 32 bytes do CSPRNG, mostrado UMA vez, guardado so como SHA-256 e
//!   comparado em tempo constante. Nunca o Bearer da API, nunca na URL que vai ao servidor
//!   (a tela o leva no fragmento `#convite=`), nunca no ledger.
//! - **Expiracao dura** (padrao 1 h, teto 8 h), conferida na entrada e a cada 250 ms pelos
//!   dois lados; **revogacao imediata** (a rota so responde depois de o laco do anfitriao
//!   tirar o convite do mapa e derrubar os convidados); **teto de convidados** (padrao 4,
//!   teto 16). Tudo no ledger da sessao (`<tarefas>/<sessao>/evidence.jsonl`), sem segredo.
//! - **Pre-requisito R1**: o terminal compartilhado nao herda o token. O `hx` aberto pela tela
//!   leva `PHXCLAW_API_TOKEN` para a completacao por IA, e um `:sh` o mostraria a todos os
//!   convidados. Por isso so compartilha o terminal aberto com `compartilhavel: true`, que
//!   nasce SEM a credencial (`ide::ambiente_do_helix`); o outro recusa dizendo que precisa
//!   reabrir. Envolver nao basta: um terminal que ja tem o token no ambiente nao o perde.
//! - Limites declarados (ordem do dono): sem edicao simultanea (CRDT/OT), sem voz, sem chat,
//!   um terminal por sessao. Sem `ide.compartilhar` no catalogo: a porta e a rota do dono e o
//!   gesto explicito dele em cada sessao.

use crate::api::{ApiState, auth};
use axum::extract::State;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::http::{HeaderMap, Method, StatusCode, header};
use axum::response::Response;
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::{DateTime, Utc};
use futures_util::SinkExt;
use phxclaw_evidence_ledger::{EvidenceDraft, EvidenceLedger, EvidenceOutcome};
use phxclaw_terminal::{Tecla, Terminal};
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, oneshot};

/// Criar o compartilhamento do terminal aberto por quem pede (o dono, pela matriz).
pub const ROTA_COMPARTILHAR: &str = "/v1/ide/compartilhar";
/// Revogar um convidado, ou o compartilhamento inteiro.
pub const ROTA_REVOGAR: &str = "/v1/ide/compartilhar/revogar";
/// O websocket do convidado: o convite vai na PRIMEIRA mensagem.
pub const ROTA_CONVIDADO: &str = "/v1/ide/compartilhado";

pub const EXPIRA_PADRAO_S: u64 = 3600;
pub const EXPIRA_TETO_S: u64 = 8 * 3600;
pub const CONVIDADOS_PADRAO: usize = 4;
pub const CONVIDADOS_TETO: usize = 16;
/// Mensagens de grade na fila de um convidado: estourou, o convidado lento sai -- ele nunca
/// segura o anfitriao (R5).
const FILA_DO_CONVIDADO: usize = 256;
/// Quanto tempo a conexao do convidado tem para mandar o convite.
const PRAZO_DO_CONVITE: Duration = Duration::from_secs(10);

pub fn rotas() -> Router<ApiState> {
    Router::new()
        .route(ROTA_COMPARTILHAR, post(compartilhar))
        .route(ROTA_REVOGAR, post(revogar))
        .route(ROTA_CONVIDADO, get(convidado))
}

type Erro = (StatusCode, Json<Value>);

fn erro(code: StatusCode, msg: impl Into<String>) -> Erro {
    (code, Json(json!({"error": msg.into()})))
}

// ---------------------------------------------------------------- estado

/// O que o laco do anfitriao recebe de fora: das rotas (criar, revogar) e do convidado que
/// escreve (entrada). O convidado de leitura nao tem este canal.
pub enum Controle {
    Criar {
        opcoes: Opcoes,
        resposta: oneshot::Sender<Result<Value, Erro>>,
    },
    Revogar {
        convidado: Option<String>,
        resposta: oneshot::Sender<Value>,
    },
    Entrada {
        convidado: String,
        entrada: Entrada,
    },
}

/// O que um convidado com escrita pode mandar: as mesmas tres entradas de texto do terminal
/// do anfitriao. Tamanho e rolagem nao: o tamanho e o do anfitriao.
#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "lowercase")]
pub enum Entrada {
    Tecla { tecla: Tecla },
    Colar { texto: String },
    Texto { texto: String },
}

/// Opcoes do convite, do corpo do `POST /v1/ide/compartilhar`.
#[derive(Deserialize, Default)]
pub struct Opcoes {
    #[serde(default)]
    pub expira_em_s: Option<u64>,
    #[serde(default)]
    pub max_convidados: Option<usize>,
    /// Conceder escrita a convidado que prove o papel do terminal pelo RBAC. Padrao: nao.
    #[serde(default)]
    pub escrita: bool,
}

/// Um convite vivo. O token nao: so o resumo.
struct Convite {
    resumo: [u8; 32],
    expira: Instant,
    expira_em: DateTime<Utc>,
    max: usize,
    escrita: bool,
    /// Convidados conectados (o teto conta estes).
    ocupados: usize,
    /// Para registrar o convidado que entra e, se ele escreve, dar-lhe a entrada.
    controle: mpsc::UnboundedSender<Controle>,
    entrou: mpsc::UnboundedSender<Convidado>,
    ledger: Option<EvidenceLedger>,
}

fn convites() -> &'static Mutex<HashMap<String, Convite>> {
    static C: OnceLock<Mutex<HashMap<String, Convite>>> = OnceLock::new();
    C.get_or_init(|| Mutex::new(HashMap::new()))
}

/// O laco de cada anfitriao vivo, pela chave do usuario (o resumo do token, `ide`).
fn anfitrioes() -> &'static Mutex<HashMap<String, mpsc::UnboundedSender<Controle>>> {
    static A: OnceLock<Mutex<HashMap<String, mpsc::UnboundedSender<Controle>>>> = OnceLock::new();
    A.get_or_init(|| Mutex::new(HashMap::new()))
}

fn travar<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

fn resumo(token: &str) -> [u8; 32] {
    use sha2::{Digest, Sha256};
    Sha256::digest(token.as_bytes()).into()
}

/// Comparacao sem saida antecipada: o tempo nao diz quantos bytes casaram.
fn iguais(a: &[u8; 32], b: &[u8; 32]) -> bool {
    a.iter().zip(b).fold(0u8, |d, (x, y)| d | (x ^ y)) == 0
}

fn anotar(ledger: Option<&EvidenceLedger>, acao: &str, desfecho: EvidenceOutcome, resumo: Value) {
    if let Some(l) = ledger {
        let _ = l.append(EvidenceDraft {
            action_uuid: phxclaw_types::new_uuid_v7(),
            correlation_uuid: None,
            actor: "phxclaw-agent.ide".into(),
            capability: "ide.compartilhar".into(),
            action: acao.into(),
            outcome: desfecho,
            request_summary: resumo,
            result_summary: Value::Null,
            artifact_uris: vec![],
        });
    }
}

/// Um convidado conectado, como o anfitriao o ve.
pub struct Convidado {
    id: String,
    escreve: bool,
    desde: DateTime<Utc>,
    fila: mpsc::Sender<Arc<str>>,
}

// ---------------------------------------------------------------- lado do anfitriao

/// O terminal aberto, como o laco do anfitriao o empresta: id, motor e se nasceu sem a
/// credencial da completacao (o unico que pode ser compartilhado).
pub struct Aberto<'a> {
    pub id: &'a str,
    pub t: &'a Terminal,
    pub compartilhavel: bool,
}

/// O lado do anfitriao, dono dos convidados. Vive dentro de `ide::sessao`; ao cair (conexao
/// fechada, sessao substituida), encerra o compartilhamento e derruba quem assistia.
pub struct Anfitriao {
    chave: String,
    controle: mpsc::UnboundedSender<Controle>,
    entrou: mpsc::UnboundedSender<Convidado>,
    sessao: Option<String>,
    convidados: Vec<Convidado>,
    ledger: Option<EvidenceLedger>,
}

/// Os dois canais que o laco do anfitriao escuta.
pub struct Escuta {
    pub controle: mpsc::UnboundedReceiver<Controle>,
    pub entrou: mpsc::UnboundedReceiver<Convidado>,
}

impl Anfitriao {
    /// Registra o laco de `chave` (a conexao nova do mesmo usuario toma o lugar da antiga).
    pub fn novo(chave: String) -> (Self, Escuta) {
        let (controle, rc) = mpsc::unbounded_channel();
        let (entrou, re) = mpsc::unbounded_channel();
        travar(anfitrioes()).insert(chave.clone(), controle.clone());
        (
            Self {
                chave,
                controle,
                entrou,
                sessao: None,
                convidados: vec![],
                ledger: None,
            },
            Escuta {
                controle: rc,
                entrou: re,
            },
        )
    }

    pub fn ativo(&self) -> bool {
        self.sessao.is_some()
    }

    /// A grade, ja serializada, a cada convidado. Fila cheia ou fechada: o convidado sai.
    pub fn difundir(&mut self, texto: &Arc<str>) -> Option<Value> {
        if self.convidados.is_empty() {
            return None;
        }
        let antes = self.convidados.len();
        self.convidados
            .retain(|c| c.fila.try_send(texto.clone()).is_ok());
        (self.convidados.len() != antes).then(|| self.estado(None))
    }

    /// O que a tela do anfitriao mostra: ativo, quando expira, quantos assistem.
    fn estado(&self, motivo: Option<&str>) -> Value {
        let c = self.sessao.as_ref().and_then(|s| {
            travar(convites())
                .get(s)
                .map(|c| (c.expira_em, c.max, c.escrita))
        });
        json!({
            "ev": "compartilhamento",
            "ativo": c.is_some(),
            "sessao": self.sessao,
            "expira_em": c.map(|c| c.0.to_rfc3339()),
            "max_convidados": c.map(|c| c.1),
            "escrita": c.map(|c| c.2),
            "convidados": self.convidados.iter().map(|c| json!({
                "id": c.id, "escreve": c.escreve, "desde": c.desde.to_rfc3339()
            })).collect::<Vec<_>>(),
            "motivo": motivo,
        })
    }

    /// Fim do compartilhamento: o convite sai do mapa (ninguem mais entra) e cada convidado
    /// recebe o motivo e perde a fila.
    pub fn encerrar(&mut self, motivo: &str) -> Option<Value> {
        let sessao = self.sessao.take()?;
        travar(convites()).remove(&sessao);
        let fim: Arc<str> = json!({"ev": "fim", "motivo": motivo}).to_string().into();
        let n = self.convidados.len();
        for c in self.convidados.drain(..) {
            let _ = c.fila.try_send(fim.clone());
        }
        anotar(
            self.ledger.as_ref(),
            "ide.compartilhar: encerrar",
            EvidenceOutcome::Succeeded,
            json!({"sessao": sessao, "motivo": motivo, "convidados": n}),
        );
        self.ledger = None;
        Some(self.estado(Some(motivo)))
    }

    /// A cada tique do laco: convite vencido encerra; convidado que fechou a conexao sai da
    /// lista (e o anfitriao ve o numero descer).
    pub fn vigiar(&mut self) -> Option<Value> {
        let sessao = self.sessao.clone()?;
        let vencido = travar(convites())
            .get(&sessao)
            .is_none_or(|c| Instant::now() >= c.expira);
        if vencido {
            return self.encerrar("expirado");
        }
        let antes = self.convidados.len();
        self.convidados.retain(|c| !c.fila.is_closed());
        (self.convidados.len() != antes).then(|| self.estado(None))
    }

    /// Um convidado admitido: recebe o retrato inteiro antes de qualquer diferenca.
    pub fn receber(&mut self, c: Convidado, aberto: Option<Aberto<'_>>) -> Option<Value> {
        // Sem compartilhamento ativo (encerrado entre a admissao e aqui), a fila cai com `c`.
        self.sessao.as_ref()?;
        if let Some(a) = aberto {
            let mut v = serde_json::to_value(a.t.grade()).unwrap_or(Value::Null);
            v["ev"] = json!("grade");
            v["id"] = json!(a.id);
            if c.fila.try_send(v.to_string().into()).is_err() {
                return None;
            }
        }
        self.convidados.push(c);
        Some(self.estado(None))
    }

    /// Criar, revogar e a entrada do convidado que escreve. Devolve o aviso para a tela do
    /// anfitriao, quando muda o que ela mostra.
    pub fn tratar(
        &mut self,
        c: Controle,
        aberto: Option<Aberto<'_>>,
        s: &ApiState,
    ) -> Option<Value> {
        match c {
            Controle::Criar { opcoes, resposta } => {
                let r = self.criar(opcoes, aberto, s);
                let aviso = r.is_ok().then(|| self.estado(None));
                let _ = resposta.send(r);
                aviso
            }
            Controle::Revogar {
                convidado,
                resposta,
            } => {
                let sessao = self.sessao.clone();
                let (aviso, n) = match convidado {
                    None => {
                        let n = self.convidados.len();
                        (self.encerrar("revogado"), n)
                    }
                    Some(id) => {
                        let fim: Arc<str> = json!({"ev": "fim", "motivo": "revogado"})
                            .to_string()
                            .into();
                        let antes = self.convidados.len();
                        self.convidados.retain(|c| {
                            if c.id == id {
                                let _ = c.fila.try_send(fim.clone());
                                false
                            } else {
                                true
                            }
                        });
                        let n = antes - self.convidados.len();
                        if n > 0 {
                            anotar(
                                self.ledger.as_ref(),
                                "ide.compartilhar: revogar convidado",
                                EvidenceOutcome::Succeeded,
                                json!({"sessao": sessao, "convidado": id}),
                            );
                        }
                        ((n > 0).then(|| self.estado(None)), n)
                    }
                };
                let _ = resposta.send(json!({"sessao": sessao, "revogados": n}));
                aviso
            }
            Controle::Entrada { convidado, entrada } => {
                // A segunda conferencia: revogado ou rebaixado depois de entrar nao digita.
                let pode = self
                    .convidados
                    .iter()
                    .any(|c| c.id == convidado && c.escreve);
                if let (true, Some(a)) = (pode, aberto) {
                    let _ = match entrada {
                        Entrada::Tecla { tecla } => a.t.tecla(&tecla).map(|_| ()),
                        Entrada::Colar { texto } => a.t.colar(&texto),
                        Entrada::Texto { texto } => a.t.escrever(texto.as_bytes()),
                    };
                }
                None
            }
        }
    }

    fn criar(
        &mut self,
        o: Opcoes,
        aberto: Option<Aberto<'_>>,
        s: &ApiState,
    ) -> Result<Value, Erro> {
        let Some(a) = aberto else {
            return Err(erro(
                StatusCode::CONFLICT,
                "nenhum terminal aberto para compartilhar",
            ));
        };
        if !a.compartilhavel {
            // R1: o hx deste terminal tem a credencial da completacao no ambiente, e um `:sh`
            // a mostraria a quem assiste. Nao ha como tira-la de um processo vivo.
            return Err((
                StatusCode::CONFLICT,
                Json(json!({
                    "error": "este Helix foi aberto com a credencial da completação por IA no ambiente; reabra-o compartilhável (sem a credencial) para compartilhar",
                    "reabrir": true,
                })),
            ));
        }
        // Um compartilhamento por anfitriao: o novo derruba o anterior.
        self.encerrar("substituido");
        let mut bytes = [0u8; 32];
        getrandom::fill(&mut bytes).map_err(|e| {
            erro(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("CSPRNG do sistema indisponivel: {e}"),
            )
        })?;
        let token: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
        let sessao = phxclaw_types::new_uuid_v7().to_string();
        let segundos = o
            .expira_em_s
            .unwrap_or(EXPIRA_PADRAO_S)
            .clamp(1, EXPIRA_TETO_S);
        let max = o
            .max_convidados
            .unwrap_or(CONVIDADOS_PADRAO)
            .clamp(1, CONVIDADOS_TETO);
        let expira_em = Utc::now() + chrono::Duration::seconds(segundos as i64);
        let ledger = EvidenceLedger::open(s.store.evidence_path(&sessao)).ok();
        anotar(
            ledger.as_ref(),
            "ide.compartilhar: criar",
            EvidenceOutcome::Succeeded,
            json!({"sessao": sessao, "terminal": a.id, "expira_em": expira_em.to_rfc3339(),
                   "max_convidados": max, "escrita": o.escrita}),
        );
        travar(convites()).insert(
            sessao.clone(),
            Convite {
                resumo: resumo(&token),
                expira: Instant::now() + Duration::from_secs(segundos),
                expira_em,
                max,
                escrita: o.escrita,
                ocupados: 0,
                controle: self.controle.clone(),
                entrou: self.entrou.clone(),
                ledger: ledger.clone(),
            },
        );
        self.sessao = Some(sessao.clone());
        self.ledger = ledger;
        Ok(json!({
            "sessao": sessao,
            "token": token,
            "convite": format!("{sessao}.{token}"),
            "expira_em": expira_em.to_rfc3339(),
            "expira_em_s": segundos,
            "max_convidados": max,
            "escrita": o.escrita,
        }))
    }
}

impl Drop for Anfitriao {
    fn drop(&mut self) {
        self.encerrar("encerrado");
        let mut a = travar(anfitrioes());
        if a.get(&self.chave)
            .is_some_and(|c| c.same_channel(&self.controle))
        {
            a.remove(&self.chave);
        }
    }
}

// ---------------------------------------------------------------- rotas do anfitriao

fn bearer(h: &HeaderMap) -> Option<&str> {
    h.get(header::AUTHORIZATION)?
        .to_str()
        .ok()?
        .strip_prefix("Bearer ")
}

/// O laco do anfitriao de quem pede: o mesmo usuario que abriu o terminal (a chave e o
/// resumo do MESMO token que a sessao do websocket recebeu).
fn laco_de(h: &HeaderMap) -> Result<mpsc::UnboundedSender<Controle>, Erro> {
    let chave = crate::ide::chave_do_usuario(bearer(h).unwrap_or(""));
    travar(anfitrioes()).get(&chave).cloned().ok_or_else(|| {
        erro(
            StatusCode::CONFLICT,
            "nenhum terminal do IDE aberto por este usuario: abra o Helix na tela IDE antes",
        )
    })
}

async fn compartilhar(
    State(s): State<ApiState>,
    h: HeaderMap,
    corpo: Option<Json<Opcoes>>,
) -> Result<Json<Value>, Erro> {
    auth(&s, &h)?;
    let laco = laco_de(&h)?;
    let (tx, rx) = oneshot::channel();
    let opcoes = corpo.map(|Json(o)| o).unwrap_or_default();
    laco.send(Controle::Criar {
        opcoes,
        resposta: tx,
    })
    .map_err(|_| {
        erro(
            StatusCode::CONFLICT,
            "a sessao do terminal acabou de fechar",
        )
    })?;
    rx.await
        .map_err(|_| {
            erro(
                StatusCode::CONFLICT,
                "a sessao do terminal acabou de fechar",
            )
        })?
        .map(Json)
}

#[derive(Deserialize, Default)]
struct PedidoDeRevogar {
    #[serde(default)]
    convidado: Option<String>,
}

/// Revoga um convidado (`convidado`) ou o compartilhamento inteiro. Responde DEPOIS de o laco
/// do anfitriao tirar o convite do mapa e derrubar as filas: quem recebe o 200 sabe que
/// ninguem mais assiste.
async fn revogar(
    State(s): State<ApiState>,
    h: HeaderMap,
    corpo: Option<Json<PedidoDeRevogar>>,
) -> Result<Json<Value>, Erro> {
    auth(&s, &h)?;
    let laco = laco_de(&h)?;
    let (tx, rx) = oneshot::channel();
    let convidado = corpo.and_then(|Json(p)| p.convidado);
    laco.send(Controle::Revogar {
        convidado,
        resposta: tx,
    })
    .map_err(|_| {
        erro(
            StatusCode::CONFLICT,
            "a sessao do terminal acabou de fechar",
        )
    })?;
    rx.await.map(Json).map_err(|_| {
        erro(
            StatusCode::CONFLICT,
            "a sessao do terminal acabou de fechar",
        )
    })
}

// ---------------------------------------------------------------- lado do convidado

#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "lowercase")]
enum PedidoDoConvidado {
    Entrar {
        sessao: String,
        token: String,
        /// Credencial da API (Bearer de usuario ou o `api.token`), so para pedir escrita.
        #[serde(default)]
        credencial: Option<String>,
    },
}

async fn convidado(State(s): State<ApiState>, ws: WebSocketUpgrade) -> Response {
    ws.on_upgrade(move |sock| sessao_do_convidado(s, sock))
}

async fn mandar(sock: &mut WebSocket, v: Value) -> bool {
    sock.send(Message::Text(v.to_string().into())).await.is_ok()
}

/// O que a admissao entrega a tarefa do convidado. `entrada` so existe para quem escreve.
struct Admissao {
    id: String,
    escreve: bool,
    fila: mpsc::Receiver<Arc<str>>,
    entrada: Option<mpsc::UnboundedSender<Controle>>,
    expira: Instant,
    expira_em: DateTime<Utc>,
    ledger: Option<EvidenceLedger>,
}

/// Confere o convite e registra o convidado no laco do anfitriao, tudo sob a trava do mapa:
/// duas entradas simultaneas nao passam juntas pelo ultimo lugar.
fn admitir(sessao: &str, token: &str, pode_escrever: bool) -> Result<Admissao, String> {
    let mut mapa = travar(convites());
    let Some(c) = mapa.get_mut(sessao) else {
        return Err("convite inexistente, revogado ou encerrado".into());
    };
    let recusa = |c: &Convite, motivo: &str| {
        anotar(
            c.ledger.as_ref(),
            "ide.compartilhar: entrar",
            EvidenceOutcome::Denied,
            json!({"sessao": sessao, "motivo": motivo}),
        );
        motivo.to_string()
    };
    if Instant::now() >= c.expira {
        return Err(recusa(c, "convite expirado"));
    }
    if !iguais(&c.resumo, &resumo(token)) {
        return Err(recusa(c, "token do convite invalido"));
    }
    if c.ocupados >= c.max {
        return Err(recusa(c, "limite de convidados atingido"));
    }
    let escreve = c.escrita && pode_escrever;
    let id = phxclaw_types::new_uuid_v7().to_string();
    let (fila_tx, fila) = mpsc::channel(FILA_DO_CONVIDADO);
    let novo = Convidado {
        id: id.clone(),
        escreve,
        desde: Utc::now(),
        fila: fila_tx,
    };
    if c.entrou.send(novo).is_err() {
        return Err("a sessao do anfitriao acabou de fechar".into());
    }
    c.ocupados += 1;
    anotar(
        c.ledger.as_ref(),
        "ide.compartilhar: entrar",
        EvidenceOutcome::Succeeded,
        json!({"sessao": sessao, "convidado": id, "escreve": escreve}),
    );
    Ok(Admissao {
        id,
        escreve,
        fila,
        entrada: escreve.then(|| c.controle.clone()),
        expira: c.expira,
        expira_em: c.expira_em,
        ledger: c.ledger.clone(),
    })
}

/// O convite ainda vale? (existe e nao venceu) -- conferido pelo convidado a cada tique,
/// sem depender do anfitriao: expiracao dura vale mesmo com o laco dele ocupado.
fn ainda_vale(sessao: &str) -> Result<(), &'static str> {
    match travar(convites()).get(sessao) {
        None => Err("revogado"),
        Some(c) if Instant::now() >= c.expira => Err("expirado"),
        Some(_) => Ok(()),
    }
}

async fn sessao_do_convidado(s: ApiState, mut sock: WebSocket) {
    let primeira = tokio::time::timeout(PRAZO_DO_CONVITE, sock.recv()).await;
    let pedido = match primeira {
        Ok(Some(Ok(Message::Text(t)))) => {
            serde_json::from_str::<PedidoDoConvidado>(t.as_str()).ok()
        }
        _ => None,
    };
    let Some(PedidoDoConvidado::Entrar {
        sessao,
        token,
        credencial,
    }) = pedido
    else {
        let _ = mandar(&mut sock, json!({"ev": "erro", "erro": "convite ausente"})).await;
        let _ = sock.close().await;
        return;
    };
    // A escrita pelo RBAC: a mesma decisao da rota do terminal, com a credencial que o
    // convidado trouxe. Fora da trava do mapa (le o arquivo de usuarios).
    let pode_escrever = credencial.as_deref().is_some_and(|c| {
        let mut h = HeaderMap::new();
        let Ok(v) = format!("Bearer {c}").parse() else {
            return false;
        };
        h.insert(header::AUTHORIZATION, v);
        crate::rbac::conferir_rota(&s, Method::GET, crate::ide::ROTA_TERMINAL, &h).is_ok()
    });
    let a = match admitir(&sessao, &token, pode_escrever) {
        Ok(a) => a,
        Err(e) => {
            let _ = mandar(&mut sock, json!({"ev": "erro", "erro": e})).await;
            let _ = sock.close().await;
            return;
        }
    };
    let Admissao {
        id,
        escreve,
        mut fila,
        entrada,
        expira,
        expira_em,
        ledger,
    } = a;
    let entrou = json!({"ev": "entrou", "sessao": sessao, "convidado": id, "escreve": escreve,
                        "expira_em": expira_em.to_rfc3339()});
    let mut motivo = "saiu";
    if mandar(&mut sock, entrou).await {
        let mut relogio = tokio::time::interval(Duration::from_millis(250));
        loop {
            tokio::select! {
                m = fila.recv() => {
                    // Fila solta pelo anfitriao: o motivo ja veio como ultima mensagem.
                    let Some(m) = m else { motivo = "encerrado"; break };
                    let fim = m.starts_with("{\"ev\":\"fim\"");
                    if sock.send(Message::Text(m.to_string().into())).await.is_err() { break }
                    if fim { motivo = "encerrado"; break }
                }
                m = sock.recv() => {
                    let Some(Ok(m)) = m else { break };
                    let texto = match m {
                        Message::Text(t) => t,
                        Message::Close(_) => break,
                        _ => continue,
                    };
                    // Quem le nao tem `entrada`: nao ha caminho ate o terminal para ignorar.
                    match &entrada {
                        Some(e) => match serde_json::from_str::<Entrada>(texto.as_str()) {
                            Ok(x) => { let _ = e.send(Controle::Entrada { convidado: id.clone(), entrada: x }); }
                            Err(_) => { if !mandar(&mut sock, json!({"ev": "erro", "erro": "pedido invalido"})).await { break } }
                        },
                        None => {
                            if !mandar(&mut sock, json!({"ev": "erro", "erro": "convite somente leitura: o convidado não digita"})).await { break }
                        }
                    }
                }
                _ = relogio.tick() => {
                    let vale = if Instant::now() >= expira { Err("expirado") } else { ainda_vale(&sessao) };
                    if let Err(m) = vale {
                        let _ = mandar(&mut sock, json!({"ev": "fim", "motivo": m})).await;
                        motivo = m;
                        break;
                    }
                }
            }
        }
    }
    if let Some(c) = travar(convites()).get_mut(&sessao) {
        c.ocupados = c.ocupados.saturating_sub(1);
    }
    anotar(
        ledger.as_ref(),
        "ide.compartilhar: sair",
        EvidenceOutcome::Succeeded,
        json!({"sessao": sessao, "convidado": id, "motivo": motivo}),
    );
    let _ = sock.close().await;
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn o_resumo_compara_em_tempo_constante_e_nao_e_o_token() {
        let a = resumo("abc");
        assert!(iguais(&a, &resumo("abc")));
        assert!(!iguais(&a, &resumo("abd")));
        let hex: String = a.iter().map(|b| format!("{b:02x}")).collect();
        assert!(!hex.contains("abc"));
    }

    /// Convite que nao existe, token errado e expirado sao recusados antes de qualquer
    /// registro; o teto conta os ocupados.
    #[test]
    fn a_admissao_confere_existencia_token_prazo_e_teto() {
        let (controle, _rc) = mpsc::unbounded_channel();
        let (entrou, mut re) = mpsc::unbounded_channel();
        let sessao = phxclaw_types::new_uuid_v7().to_string();
        travar(convites()).insert(
            sessao.clone(),
            Convite {
                resumo: resumo("certo"),
                expira: Instant::now() + Duration::from_secs(60),
                expira_em: Utc::now(),
                max: 1,
                escrita: true,
                ocupados: 0,
                controle,
                entrou,
                ledger: None,
            },
        );
        assert!(admitir("nao-existe", "certo", false).is_err());
        assert_eq!(
            admitir(&sessao, "errado", false).err().unwrap(),
            "token do convite invalido"
        );
        let a = admitir(&sessao, "certo", false).unwrap();
        // Convite com escrita, mas sem a prova do RBAC: entra lendo, sem entrada.
        assert!(!a.escreve && a.entrada.is_none());
        assert!(re.try_recv().is_ok());
        assert_eq!(
            admitir(&sessao, "certo", true).err().unwrap(),
            "limite de convidados atingido"
        );
        travar(convites()).get_mut(&sessao).unwrap().ocupados = 0;
        let b = admitir(&sessao, "certo", true).unwrap();
        assert!(b.escreve && b.entrada.is_some());
        travar(convites()).get_mut(&sessao).unwrap().expira = Instant::now();
        assert_eq!(
            admitir(&sessao, "certo", false).err().unwrap(),
            "convite expirado"
        );
        travar(convites()).remove(&sessao);
    }
}
