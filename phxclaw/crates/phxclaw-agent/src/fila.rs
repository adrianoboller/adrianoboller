//! Modo fila (o «queue mode» do n8n): `phxclaw servir --modo fila` poe cada execucao numa
//! fila no PostgreSQL, e N processos `phxclaw worker` as tiram de la e executam.
//!
//! Decisoes que valem saber:
//! - **Nenhuma fila nova e nenhum motor novo.** A fila e a do `phxclaw-task-graph`
//!   (`PostgresTaskJournal` sobre `phoenix_tasks`/`phoenix_task_runs`/`phoenix_task_events`,
//!   migracao 0004), que o agente ja usa para o grafo dos fluxos e cujo comentario de tabela
//!   ja dizia «workers should claim ready rows using FOR UPDATE SKIP LOCKED». O worker
//!   executa pelas MESMAS funcoes do modo normal (`api::executar_aqui`, `planejar_aqui`,
//!   `rodar_fluxo_aqui`, `retomar_fluxo_aqui`); a unica decisao «fila ou aqui» mora nas
//!   portas da `api.rs`, e e por isso que o resultado e o mesmo.
//! - **A posse e o `next_eligible_at`** da linha `running`, empurrado por batimento a cada
//!   terco do prazo; o `active_run_uuid` e a cerca. Worker morto para de bater, a posse
//!   vence e outro toma. Fluxo tomado de novo RETOMA pelo progresso gravado a cada onda (o
//!   que terminou bem nao roda outra vez); tarefa de objetivo recomeca (pelo menos uma vez,
//!   como o n8n). Depois de `fila.tentativas` tomadas vencidas, `dead_letter` e a tarefa FALHA
//!   dizendo por que.
//! - **A pasta das tarefas e compartilhada** entre o `servir` e os workers (mesmo host ou
//!   volume): o PostgreSQL decide QUEM roda, o disco continua sendo onde a execucao mora (a
//!   evidencia, os artefatos, o `task.json` que o `GET /v1/tasks/{id}` le). Diverge do n8n,
//!   que guarda a execucao no banco: aqui a pasta da tarefa ja e a porta de disco das
//!   ferramentas (`confine`), e copiar para o banco seria a segunda casa do mesmo dado.
//! - **Senha so do operador:** `fila.url` (so do operador; pode trazer a senha) ou a senha no
//!   broker (`phxclaw fila senha`). Nenhuma mensagem de erro carrega a senha.
//! - **Desligado nao custa nada:** sem `--modo fila` nenhum registro existe, e a porta da
//!   `api.rs` pergunta a um `AtomicBool` antes de qualquer trava.

use crate::api::ApiState;
use crate::motor::Agent;
use crate::tarefa::{Task, TaskStatus, TaskStore};
use phxclaw_task_graph::{
    CancelRequest, Heartbeat, PostgresTaskJournal, QueueClaim, QueueLease, RunOutcome,
    TaskGraphError, TaskSpec, TaskStatus as Linha,
};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// As capacidades (coluna `capability`) que a fila do agente usa: a mesma tabela serve outros
/// consumidores do `task-graph`, e o worker so toma as dele.
pub const CAPACIDADES: [&str; 4] = [
    "phxclaw.tarefa",
    "phxclaw.plano",
    "phxclaw.fluxo",
    "phxclaw.retomada",
];

/// Prazo da posse padrao: o `QUEUE_WORKER_LOCK_DURATION` do n8n (30 s), com o batimento a
/// cada terco (o n8n renova na metade; um terco tolera perder um batimento).
pub const PRAZO_PADRAO: Duration = Duration::from_secs(30);

/// `fila.senha`: a senha do PostgreSQL da fila, no broker de `<pasta>/fila`.
pub const SENHA: crate::chaves::Servico = crate::chaves::Servico {
    espaco: "fila",
    nome_do_segredo: "fila-senha",
    chave: "fila.senha",
    aliases: &[],
    rotulo: "a senha do PostgreSQL da fila",
    comando: "fila senha",
};

/// O que vai para a fila.
pub enum Trabalho {
    Tarefa,
    Plano,
    /// A definicao viaja inteira: o arquivo pode mudar entre o disparo e a tomada.
    Fluxo {
        fluxo: Value,
        entrada: Vec<Value>,
        ate: Option<String>,
    },
}

/// A conexao com a fila. Sem `Debug` de proposito: a configuracao carrega a senha.
pub struct Fila {
    conexao: postgres::Config,
    senha: Option<String>,
    pub prazo: Duration,
    pub tentativas: u16,
}

impl Fila {
    /// `url` no formato do libpq (URL ou `chave=valor`); `senha` entra se a URL nao tem.
    pub fn nova(url: &str, senha: Option<&str>) -> Result<Self, String> {
        let mut conexao: postgres::Config = url.parse().map_err(|_| {
            "fila.url invalida (formato do libpq: postgresql://usuario@host:porta/banco)"
                .to_string()
        })?;
        let mut guardada = conexao
            .get_password()
            .map(|p| String::from_utf8_lossy(p).into_owned());
        if guardada.is_none()
            && let Some(s) = senha.filter(|s| !s.is_empty())
        {
            conexao.password(s);
            guardada = Some(s.to_string());
        }
        conexao.connect_timeout(Duration::from_secs(5));
        Ok(Self {
            conexao,
            senha: guardada,
            prazo: PRAZO_PADRAO,
            tentativas: 3,
        })
    }

    /// A fila do `config.json` do operador; `None` sem `fila.url`.
    pub fn do_config(raiz_do_agente: &Path) -> Result<Option<Self>, String> {
        let Some(url) = crate::config::texto_de("fila.url").filter(|u| !u.trim().is_empty()) else {
            return Ok(None);
        };
        let senha = SENHA.do_ambiente_ou_broker(raiz_do_agente)?;
        let mut f = Self::nova(url.trim(), senha.as_deref())?;
        if let Some(s) = crate::config::inteiro_de("fila.prazo_segundos").filter(|s| *s > 0) {
            f.prazo = Duration::from_secs(s as u64);
        }
        if let Some(n) = crate::config::inteiro_de("fila.tentativas").filter(|n| *n > 0) {
            f.tentativas = u16::try_from(n).unwrap_or(u16::MAX);
        }
        Ok(Some(f))
    }

    fn prazo_ms(&self) -> i64 {
        i64::try_from(self.prazo.as_millis())
            .unwrap_or(i64::MAX)
            .max(1)
    }

    /// Roda `f` com uma conexao nova, numa thread propria: o cliente bloqueante do
    /// `postgres` sobe o proprio runtime e entraria em panico dentro de uma tarefa do tokio.
    /// O erro sai sem a senha.
    fn com_cliente<T: Send>(
        &self,
        f: impl FnOnce(&mut postgres::Client) -> Result<T, TaskGraphError> + Send,
    ) -> Result<T, String> {
        let conexao = &self.conexao;
        let r = std::thread::scope(|s| {
            s.spawn(move || {
                let mut c = conexao
                    .connect(postgres::NoTls)
                    .map_err(|e| format!("PostgreSQL da fila: {e}"))?;
                f(&mut c).map_err(|e| format!("fila: {e}"))
            })
            .join()
            .unwrap_or_else(|_| Err("fila: a thread do PostgreSQL caiu".into()))
        });
        r.map_err(|e| match &self.senha {
            Some(s) if !s.is_empty() => e.replace(s.as_str(), "***"),
            _ => e,
        })
    }

    /// As tabelas da migracao 0004 existem; senao, a recusa diz qual arquivo aplicar.
    pub fn conferir_esquema(&self) -> Result<(), String> {
        if self.com_cliente(PostgresTaskJournal::queue_schema_ready)? {
            Ok(())
        } else {
            Err(
                "o banco da fila nao tem as tabelas phoenix_tasks/phoenix_task_runs/\
phoenix_task_events: aplique migrations/0004_task_graph.sql (ou o \
database/PhxClaw_PostgreSQL_FULL_INSTALL_v0.70.sql)"
                    .into(),
            )
        }
    }

    fn tomar(&self, worker: &str) -> Result<Option<QueueClaim>, String> {
        let caps: Vec<String> = CAPACIDADES.iter().map(|c| c.to_string()).collect();
        let ms = self.prazo_ms();
        self.com_cliente(|c| PostgresTaskJournal::claim(c, worker, &caps, ms))
    }
}

// ------------------------------------------------------------------ a instancia

/// Alguma instancia deste processo esta em modo fila? A porta da `api.rs` pergunta isto
/// antes de qualquer trava: sem fila, o custo e uma leitura atomica.
static ALGUMA: AtomicBool = AtomicBool::new(false);
static INSTANCIAS: Mutex<BTreeMap<PathBuf, Arc<Fila>>> = Mutex::new(BTreeMap::new());

/// A tarja do erro antes de ir ao banco da fila: credencial redigida e, se houver PII, so a
/// classe (`[pii: email]`). Reusa os motores do segredo e do no de politica -- sem segundo
/// detector.
fn tarja_de_erro(e: &str) -> String {
    let pii = crate::fluxo_politica::achar_pii(e);
    if !pii.is_empty() {
        return format!("[erro com {}]", pii.join(", "));
    }
    phxclaw_secret_broker::scrub_secret_like(&e.chars().take(300).collect::<String>())
}

/// Poe a instancia (a raiz das tarefas) em modo fila. Chave pela raiz, como o limite de
/// fluxos: duas instancias no mesmo processo (os testes) nao se atrapalham.
pub fn ligar(raiz_das_tarefas: &Path, fila: Arc<Fila>) {
    INSTANCIAS
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .insert(raiz_das_tarefas.to_path_buf(), fila);
    ALGUMA.store(true, Ordering::Release);
}

/// Tira a instancia do modo fila.
pub fn desligar(raiz_das_tarefas: &Path) {
    let mut m = INSTANCIAS.lock().unwrap_or_else(|p| p.into_inner());
    m.remove(raiz_das_tarefas);
    ALGUMA.store(!m.is_empty(), Ordering::Release);
}

/// A fila da instancia, se ela esta em modo fila.
pub fn da_instancia(raiz_das_tarefas: &Path) -> Option<Arc<Fila>> {
    if !ALGUMA.load(Ordering::Acquire) {
        return None;
    }
    INSTANCIAS
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .get(raiz_das_tarefas)
        .cloned()
}

// ------------------------------------------------------------------ o lado do servir

fn chave(tipo: &str, id: &str) -> String {
    format!("phxclaw:{tipo}:{id}")
}

fn spec(fila: &Fila, tipo: &str, nome: &str, payload: Value, chave: String) -> TaskSpec {
    let mut s = TaskSpec::new(nome, format!("phxclaw.{tipo}"), payload, chave);
    s.retry.max_attempts = fila.tentativas.max(1);
    s
}

/// Enfileira a execucao da tarefa ja gravada. Recusa da fila deixa a tarefa FALHA no disco
/// dizendo por que (ela ja existe; `pending` para sempre seria mentira). O handle termina
/// quando o worker tira a tarefa de `pending`/`running`.
pub fn enfileirar_ou_falhar(
    fila: &Fila,
    store: &TaskStore,
    mut t: Task,
    trabalho: Trabalho,
) -> Result<tokio::task::JoinHandle<Task>, String> {
    let (tipo, payload) = match trabalho {
        Trabalho::Tarefa => ("tarefa", json!({"tipo": "tarefa", "tarefa": t.id})),
        Trabalho::Plano => ("plano", json!({"tipo": "plano", "tarefa": t.id})),
        Trabalho::Fluxo {
            fluxo,
            entrada,
            ate,
        } => (
            "fluxo",
            json!({"tipo": "fluxo", "tarefa": t.id, "fluxo": fluxo, "entrada": entrada, "ate": ate}),
        ),
    };
    let s = spec(
        fila,
        tipo,
        &t.objective.chars().take(120).collect::<String>(),
        payload,
        chave(tipo, &t.id),
    );
    if let Err(e) = fila.com_cliente(|c| PostgresTaskJournal::enqueue(c, &s)) {
        t.status = TaskStatus::Failed;
        t.error = Some(format!("fila: {e}"));
        t.updated_at = chrono::Utc::now();
        let _ = store.save(&t);
        return Err(e);
    }
    Ok(esperar_fim(store.clone(), t.id))
}

/// A retomada de uma espera. A chave e a do progresso gravado: o laco do servidor acorda a
/// mesma espera vencida a cada volta ate um worker pega-la, e o mesmo progresso nao vira
/// duas retomadas.
pub fn enfileirar_retomada(fila: &Fila, store: &TaskStore, id: &str) -> Result<(), String> {
    let t = store.load(id).map_err(|e| format!("tarefa {id}: {e}"))?;
    let marca = crate::fluxos::sha256_hex(t.answer.as_deref().unwrap_or("").as_bytes());
    let s = spec(
        fila,
        "retomada",
        &t.objective.chars().take(120).collect::<String>(),
        json!({"tipo": "retomada", "tarefa": id}),
        format!("{}:{}", chave("retomada", id), &marca[..16]),
    );
    fila.com_cliente(|c| PostgresTaskJournal::enqueue(c, &s))
        .map(|_| ())
}

/// O fim de uma execucao que corre num worker, lido do disco compartilhado: o canal que
/// espera a resposta e o `fim` de quem criou a tarefa continuam valendo em modo fila.
fn esperar_fim(store: TaskStore, id: String) -> tokio::task::JoinHandle<Task> {
    tokio::spawn(async move {
        let mut espera = Duration::from_millis(200);
        let mut faltas = 0;
        loop {
            tokio::time::sleep(espera).await;
            espera = (espera * 2).min(Duration::from_secs(2));
            match store.load(&id) {
                Ok(t) if !matches!(t.status, TaskStatus::Pending | TaskStatus::Running) => {
                    return t;
                }
                Ok(_) => faltas = 0,
                Err(e) => {
                    faltas += 1;
                    if faltas > 10 {
                        let mut t = Task::new("?", "");
                        t.id = id;
                        t.status = TaskStatus::Failed;
                        t.error = Some(format!("tarefa sumiu do disco: {e}"));
                        return t;
                    }
                }
            }
        }
    })
}

/// O que o cancelamento fez na fila.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cancelamento {
    /// A execucao corre num worker: o pedido vai pelo batimento.
    Pedido,
    /// Saiu da fila antes de comecar: quem chama marca a tarefa cancelada.
    AntesDeComecar,
    /// A fila nao tem execucao viva desta tarefa.
    ForaDaFila,
}

pub fn cancelar(fila: &Fila, t: &Task) -> Result<Cancelamento, String> {
    for tipo in ["tarefa", "plano", "fluxo"] {
        let k = chave(tipo, &t.id);
        match fila.com_cliente(|c| PostgresTaskJournal::request_cancel(c, &k))? {
            CancelRequest::Requested => return Ok(Cancelamento::Pedido),
            CancelRequest::CancelledBeforeStart => return Ok(Cancelamento::AntesDeComecar),
            CancelRequest::AlreadyFinished | CancelRequest::NotFound => {}
        }
    }
    Ok(Cancelamento::ForaDaFila)
}

// ------------------------------------------------------------------ o worker

/// O que um worker fez com uma execucao tomada.
#[derive(Debug, Clone)]
pub struct Feito {
    pub tarefa: String,
    pub tipo: String,
    pub estado: TaskStatus,
    /// A posse anterior tinha vencido: esta foi uma retomada.
    pub retomada: bool,
    /// O fim foi aceito pela fila (`false` = a posse ja era de outro; resultado descartado).
    pub aceito: bool,
    pub erro: Option<String>,
}

/// Aborta a tarefa no `Drop`: o batimento morre junto com a execucao que ele sustenta.
struct Abortar(tokio::task::JoinHandle<()>);
impl Drop for Abortar {
    fn drop(&mut self) {
        self.0.abort();
    }
}

/// Toma UMA execucao e a roda ate o fim. `None` = fila vazia.
pub async fn trabalhar_uma(
    state: &ApiState,
    fila: &Arc<Fila>,
    nome: &str,
) -> Result<Option<Feito>, String> {
    let (f, n) = (fila.clone(), nome.to_string());
    let tomada = tokio::task::spawn_blocking(move || f.tomar(&n))
        .await
        .map_err(|e| format!("fila: {e}"))??;
    match tomada {
        None => Ok(None),
        Some(c) => Ok(Some(executar_tomada(state, fila, c).await)),
    }
}

fn texto(v: &Value, campo: &str) -> Option<String> {
    v.get(campo).and_then(Value::as_str).map(str::to_string)
}

async fn executar_tomada(state: &ApiState, fila: &Arc<Fila>, c: QueueClaim) -> Feito {
    let lease = match c {
        QueueClaim::Run(l) => l,
        QueueClaim::DeadLetter(d) => {
            let id = texto(&d.payload, "tarefa").unwrap_or_default();
            if let Ok(mut t) = state.store.load(&id)
                && !t.status.is_final()
            {
                t.status = TaskStatus::Failed;
                t.error = Some(format!("fila: {}", d.error));
                t.updated_at = chrono::Utc::now();
                let _ = state.store.save(&t);
            }
            return Feito {
                tarefa: id,
                tipo: texto(&d.payload, "tipo").unwrap_or_default(),
                estado: TaskStatus::Failed,
                retomada: true,
                aceito: true,
                erro: Some(d.error),
            };
        }
    };
    let id = texto(&lease.payload, "tarefa").unwrap_or_default();
    let tipo = texto(&lease.payload, "tipo").unwrap_or_default();
    let perdeu = Arc::new(AtomicBool::new(false));
    let _batimento = Abortar(tokio::spawn(bater(
        state.clone(),
        fila.clone(),
        lease.clone(),
        id.clone(),
        perdeu.clone(),
    )));
    let r = rodar(state, &lease, &tipo, &id).await;
    let (estado, erro) = match &r {
        Ok(t) => (t.status, t.error.clone()),
        Err(e) => {
            // Recusa antes de comecar (definicao ilegivel, modelo que nao monta): a tarefa ja
            // existe e tem de dizer por que nao rodou.
            if let Ok(mut t) = state.store.load(&id)
                && !t.status.is_final()
            {
                t.status = TaskStatus::Failed;
                t.error = Some(e.clone());
                t.updated_at = chrono::Utc::now();
                let _ = state.store.save(&t);
            }
            (TaskStatus::Failed, Some(e.clone()))
        }
    };
    drop(_batimento);
    let resultado = RunOutcome {
        status: match estado {
            TaskStatus::Completed | TaskStatus::AwaitingInput | TaskStatus::AwaitingApproval => {
                Linha::Succeeded
            }
            TaskStatus::Cancelled => Linha::Cancelled,
            _ => Linha::Failed,
        },
        result: json!({"tarefa": id, "estado": estado}),
        // So o comeco: o erro inteiro esta no `task.json`, e a fila nao e o log. Passa pela
        // tarja de credencial e, se tiver PII (e-mail, CPF/CNPJ, telefone), vira so a classe:
        // o banco da fila nao e lugar de dado pessoal nem de segredo.
        error: erro.as_ref().map(|e| tarja_de_erro(e)),
    };
    let (f, l) = (fila.clone(), lease.clone());
    let aceito = !perdeu.load(Ordering::Acquire)
        && tokio::task::spawn_blocking(move || {
            f.com_cliente(|c| PostgresTaskJournal::finish(c, &l, &resultado))
        })
        .await
        .ok()
        .and_then(Result::ok)
        .unwrap_or(false);
    Feito {
        tarefa: id,
        tipo,
        estado,
        retomada: lease.reclaimed,
        aceito,
        erro,
    }
}

/// O batimento: empurra a posse a cada terco do prazo; le o pedido de cancelamento e o
/// repassa ao `CancelFlag` da tarefa (o mesmo do modo normal); posse perdida tambem cancela,
/// porque o resultado deste worker nao vai mais valer.
async fn bater(
    state: ApiState,
    fila: Arc<Fila>,
    lease: QueueLease,
    id: String,
    perdeu: Arc<AtomicBool>,
) {
    let passo = (fila.prazo / 3).max(Duration::from_millis(100));
    let ms = fila.prazo_ms();
    loop {
        tokio::time::sleep(passo).await;
        let (f, l) = (fila.clone(), lease.clone());
        let r = tokio::task::spawn_blocking(move || {
            f.com_cliente(|c| PostgresTaskJournal::renew(c, &l, ms))
        })
        .await;
        let cancelar = match r {
            Ok(Ok(Heartbeat::Held { cancel_requested })) => cancel_requested,
            Ok(Ok(Heartbeat::Lost)) => {
                perdeu.store(true, Ordering::Release);
                true
            }
            // Banco fora do ar: a posse ainda pode valer; tenta de novo no proximo passo.
            _ => false,
        };
        if cancelar && let Some(c) = state.running.lock().unwrap().get(&id) {
            c.cancel();
        }
        if perdeu.load(Ordering::Acquire) {
            return;
        }
    }
}

fn agente_do_fluxo(state: &ApiState) -> Result<Agent, String> {
    (state.factory)(&state.default_model).map(|a| crate::metricas::GLOBAL.medindo(a))
}

/// O progresso de fluxo gravado na tarefa (a execucao anterior chegou a comecar)?
fn tem_progresso(t: &Task) -> bool {
    t.answer
        .as_deref()
        .is_some_and(|a| serde_json::from_str::<crate::fluxos::Relatorio>(a).is_ok())
}

/// A execucao, pelas MESMAS funcoes do modo normal.
async fn rodar(state: &ApiState, lease: &QueueLease, tipo: &str, id: &str) -> Result<Task, String> {
    let store = &state.store;
    let t = store.load(id).map_err(|e| format!("tarefa {id}: {e}"))?;
    match tipo {
        "tarefa" => {
            let agente = crate::api::agente_da_tarefa(state, &t)?;
            crate::api::executar_aqui(state.clone(), agente, t)
                .await
                .map_err(|e| format!("execucao: {e}"))
        }
        "plano" => {
            let agente = (state.factory)(&t.model)?;
            Ok(crate::api::planejar_aqui(state.clone(), agente, t).await)
        }
        "fluxo" => {
            let def = lease.payload.get("fluxo").cloned().unwrap_or_default();
            let f = crate::fluxos::ler(&def.to_string())?;
            let entrada = lease
                .payload
                .get("entrada")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            let ate = texto(&lease.payload, "ate");
            let agente = agente_do_fluxo(state)?;
            let retomar = lease.reclaimed && tem_progresso(&t);
            Ok(crate::api::rodar_fluxo_aqui(
                store,
                &agente,
                &f,
                t,
                entrada,
                ate.as_deref(),
                retomar,
            )
            .await)
        }
        "retomada" => {
            let agente = agente_do_fluxo(state)?;
            if t.status == TaskStatus::AwaitingInput {
                crate::api::retomar_fluxo_aqui(store, &agente, id).await;
            } else if lease.reclaimed && t.status == TaskStatus::Running && tem_progresso(&t) {
                // A retomada anterior morreu no meio: a espera ja foi consumida, e o que resta
                // e o progresso por onda, com a definicao que a espera gravou na pasta.
                let arq = store.dir(id).join(crate::fluxos::ARQUIVO_DEFINICAO);
                let texto = std::fs::read_to_string(&arq)
                    .map_err(|e| format!("tarefa {id}: definicao do fluxo: {e}"))?;
                let f = crate::fluxos::ler(&texto)?;
                return Ok(
                    crate::api::rodar_fluxo_aqui(store, &agente, &f, t, vec![], None, true).await,
                );
            }
            store.load(id).map_err(|e| format!("tarefa {id}: {e}"))
        }
        outro => Err(format!("tipo de execucao desconhecido na fila: {outro}")),
    }
}

/// Como o worker roda.
pub struct Worker {
    pub nome: String,
    /// Execucoes ao mesmo tempo neste processo (o `--concurrency` do n8n).
    pub concorrencia: usize,
    /// Espera entre consultas com a fila vazia.
    pub ocioso: Duration,
}

/// O laco do `phxclaw worker`: toma enquanto houver vaga, ate `parar` virar `true`; ai para
/// de tomar e espera as execucoes em curso terminarem (elas fecham o run na fila).
pub async fn trabalhar(
    state: ApiState,
    fila: Arc<Fila>,
    w: Worker,
    mut parar: tokio::sync::watch::Receiver<bool>,
    log: impl Fn(&str) + Send + Sync + 'static,
) {
    let log = Arc::new(log);
    let n = w.concorrencia.max(1);
    let vagas = Arc::new(tokio::sync::Semaphore::new(n));
    loop {
        if *parar.borrow() {
            break;
        }
        let vaga = tokio::select! {
            v = vagas.clone().acquire_owned() => match v { Ok(v) => v, Err(_) => break },
            _ = parar.changed() => continue,
        };
        let (f, nome) = (fila.clone(), w.nome.clone());
        let tomada = tokio::task::spawn_blocking(move || f.tomar(&nome)).await;
        match tomada {
            Ok(Ok(Some(c))) => {
                let (st, f, log) = (state.clone(), fila.clone(), log.clone());
                let nome = w.nome.clone();
                tokio::spawn(async move {
                    let _vaga = vaga;
                    let x = executar_tomada(&st, &f, c).await;
                    log(&format!(
                        "worker {nome}: {} {}{} -> {:?}{}",
                        x.tipo,
                        x.tarefa,
                        if x.retomada { " (retomada)" } else { "" },
                        x.estado,
                        if x.aceito {
                            ""
                        } else {
                            " [posse perdida: resultado descartado]"
                        }
                    ));
                });
            }
            Ok(Ok(None)) => {
                drop(vaga);
                tokio::select! {
                    _ = tokio::time::sleep(w.ocioso) => {}
                    _ = parar.changed() => {}
                }
            }
            Ok(Err(e)) => {
                drop(vaga);
                log(&format!("worker {}: {e}", w.nome));
                tokio::select! {
                    _ = tokio::time::sleep(w.ocioso.max(Duration::from_secs(1))) => {}
                    _ = parar.changed() => {}
                }
            }
            Err(e) => {
                drop(vaga);
                log(&format!("worker {}: {e}", w.nome));
            }
        }
    }
    let _ = vagas.acquire_many(n as u32).await;
}

#[cfg(test)]
mod testes_tarja {
    use super::tarja_de_erro;

    // defeito reposto: antes da tarja, o erro ia cru ao banco da fila (300 chars), vazando
    // credencial e PII. O teste falha se tarja_de_erro deixar passar qualquer um dos dois.
    #[test]
    fn tarja_omite_credencial_e_pii() {
        let cred = "falhou com token ghp_A1b2C3d4E5f6G7h8I9j0K1l2M3n4O5p6Q7r8";
        let t = tarja_de_erro(cred);
        assert!(
            !t.contains("ghp_A1b2C3d4E5f6G7h8I9j0K1l2M3n4O5p6Q7r8"),
            "credencial vazou: {t}"
        );

        let email = tarja_de_erro("SMTP rejeitou joao.silva@cliente.com");
        assert_eq!(email, "[erro com email]");

        let cpf = tarja_de_erro("falha no cadastro 529.982.247-25");
        assert_eq!(cpf, "[erro com cpf]");

        // sem credencial nem PII: o texto passa, so cortado em 300 chars.
        assert_eq!(
            tarja_de_erro("timeout na porta 8787"),
            "timeout na porta 8787"
        );
    }
}
