//! Exportacao OpenTelemetry: traces (tarefa -> passos -> chamadas de ferramenta e de modelo,
//! com os ids da evidencia) e as metricas do `/metrics`, por OTLP/HTTP com JSON, para o
//! coletor do operador (`otel.url`).
//!
//! Decisoes que valem saber:
//! - **JSON, sem protobuf e sem crate nova.** O OTLP/HTTP aceita `application/json` com o
//!   mapeamento JSON do proto3 (especificacao OTLP 1.x, «JSON Protobuf Encoding»): `traceId` e
//!   `spanId` em HEXADECIMAL (16 e 8 bytes), nao base64; inteiros de 64 bits e
//!   `*UnixNano` como TEXTO decimal; enums como numero. O corpo sai do `serde_json` que ja
//!   esta em todo crate, e o envio pelo `reqwest` que o agente ja usa.
//! - **Desligado custa zero, e o interruptor vem ANTES do trabalho.** Sem `otel.url` nada e
//!   montado: o ponto de captura le um `AtomicBool` e volta, antes de abrir o `task.json`.
//!   Ligado, o trace se monta numa thread de bloqueio (le o disco) e o envio e assincrono:
//!   o fim da tarefa nao espera o coletor.
//! - **Atributo so de conjunto fechado (`ATRIBUTOS`), e nenhum valor livre do usuario.**
//!   Nem argumento de ferramenta, nem objetivo, nem texto de erro, nem nome de credencial: o
//!   argumento mora na evidencia, e o span leva o ID da evidencia para quem precisa ir la. O
//!   erro vira `status.code = ERROR` com o ESTADO como mensagem, nunca o texto.
//! - **So execucao terminada exporta** (`completed`, `failed`, `cancelled`,
//!   `budget_exceeded`): o fluxo parado numa espera exporta quando terminar, inteiro, e os
//!   ids sao deterministicos (sha256 do id da tarefa), entao reexportar nao inventa outro
//!   trace.
//! - **O comeco do span de chamada e o fim do registro anterior** (o passo e a evidencia
//!   gravam a hora em que a chamada TERMINOU): o atributo `phxclaw.inicio_aproximado` diz
//!   isso no proprio span, em vez de fingir precisao. O span de tarefa e exato (criacao e
//!   ultima gravacao).
//! - **Diverge do n8n**, que exporta pelo SDK do OpenTelemetry em Node: aqui o mesmo binario
//!   sem dependencia nova. Sem protobuf (o coletor do OpenTelemetry, o Jaeger e o Tempo
//!   aceitam JSON na 4318) e sem lote em memoria: um POST por execucao terminada.

use crate::metricas::{Numero, Serie, TipoDeSerie};
use crate::tarefa::{Task, TaskStatus, TaskStore};
use chrono::{DateTime, Utc};
use phxclaw_evidence_ledger::EvidenceRecord;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::io::BufRead;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering::Relaxed};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// As UNICAS chaves de atributo que saem daqui. Chave nova entra nesta lista, e o teste
/// reprova a que sair sem ela.
pub const ATRIBUTOS: &[&str] = &[
    "service.name",
    "service.instance.id",
    "phxclaw.papel",
    "phxclaw.tarefa.id",
    "phxclaw.tarefa.estado",
    "phxclaw.modelo",
    "phxclaw.fluxo.nome",
    "phxclaw.passo.id",
    "phxclaw.passo.n",
    "phxclaw.passo.estado",
    "phxclaw.passo.tentativas",
    "phxclaw.ferramenta.nome",
    "phxclaw.resultado",
    "phxclaw.evidencia.id",
    "phxclaw.evidencia.hash",
    "phxclaw.custo",
    "phxclaw.tokens.entrada",
    "phxclaw.tokens.saida",
    "phxclaw.inicio_aproximado",
    // As series do `/metrics` levam o rotulo delas (conjunto fechado em `metricas.rs`).
    "estado",
    "resultado",
    "tipo",
];

/// Teto de spans por trace: um fluxo de 1.000 itens com uma chamada por item cabe; um
/// trace maior que isso o coletor recusaria de qualquer forma.
pub const MAX_SPANS: usize = 4_000;

/// O destino e o nome do servico. Sem `Debug` com URL crua nao ha: a URL nao pode trazer
/// credencial (recusada em `nova`).
#[derive(Debug, Clone)]
pub struct Config {
    base: String,
    pub servico: String,
    pub intervalo: Duration,
    pub papel: String,
}

impl Config {
    /// `url` e a BASE do coletor (`http://127.0.0.1:4318`); os caminhos `/v1/traces` e
    /// `/v1/metrics` sao os da especificacao. URL com usuario/senha e recusada: credencial
    /// em configuracao seria segredo fora do broker.
    pub fn nova(url: &str, servico: &str, papel: &str) -> Result<Self, String> {
        let u = url.trim().trim_end_matches('/');
        let resto = u
            .strip_prefix("http://")
            .or_else(|| u.strip_prefix("https://"))
            .ok_or("otel.url tem de comecar com http:// ou https://")?;
        let host = resto.split('/').next().unwrap_or("");
        if host.is_empty() || host.contains('@') {
            return Err(
                "otel.url sem host, ou com usuario/senha (credencial nao entra em URL)".into(),
            );
        }
        Ok(Self {
            base: u.to_string(),
            servico: if servico.trim().is_empty() {
                "phxclaw".into()
            } else {
                servico.trim().to_string()
            },
            intervalo: Duration::from_secs(60),
            papel: papel.to_string(),
        })
    }

    /// Do `config.json` do operador; `None` sem `otel.url` (desligado).
    pub fn do_config(papel: &str) -> Result<Option<Self>, String> {
        let Some(url) = crate::config::texto_de("otel.url").filter(|u| !u.trim().is_empty()) else {
            return Ok(None);
        };
        let servico = crate::config::texto_de("otel.servico").unwrap_or_default();
        let mut c = Self::nova(&url, &servico, papel)?;
        if let Some(s) = crate::config::inteiro_de("otel.intervalo_segundos").filter(|s| *s > 0) {
            c.intervalo = Duration::from_secs(s as u64);
        }
        Ok(Some(c))
    }

    /// Para o log: so a base, que nao tem credencial.
    pub fn destino(&self) -> &str {
        &self.base
    }

    fn recurso(&self) -> Value {
        json!({"attributes": [
            atributo("service.name", Valor::Texto(self.servico.clone())),
            atributo("service.instance.id", Valor::Texto(format!("{}", std::process::id()))),
            atributo("phxclaw.papel", Valor::Texto(self.papel.clone())),
        ]})
    }
}

/// O exportador. O do processo e `GLOBAL`; o teste usa o seu, para medir sem outro teste
/// mexer no mesmo interruptor.
pub struct Exportador {
    ligado: AtomicBool,
    config: Mutex<Option<Arc<Config>>>,
    /// Traces MONTADOS (o trabalho que o interruptor desligado tem de evitar).
    pub montados: AtomicU64,
    pub enviados: AtomicU64,
    pub falhas: AtomicU64,
}

pub static GLOBAL: Exportador = Exportador::novo();

impl Default for Exportador {
    fn default() -> Self {
        Self::novo()
    }
}

/// Liga o do processo: as metricas passam a contar (sem abrir `/metrics`) e, num runtime, o
/// laco das metricas sobe.
pub fn ligar(c: Option<Config>, _raiz: &Path) {
    let Some(c) = c else {
        GLOBAL.definir(None);
        return;
    };
    crate::metricas::contar_para_exportar();
    // O comeco da serie cumulativa e o do processo, nao o do primeiro envio.
    let _ = inicio_do_processo();
    let intervalo = c.intervalo;
    GLOBAL.definir(Some(c));
    if tokio::runtime::Handle::try_current().is_ok() {
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(intervalo).await;
                if let Some(h) = GLOBAL.exportar_metricas(&crate::metricas::GLOBAL) {
                    let _ = h.await;
                }
            }
        });
    }
}

impl Exportador {
    pub const fn novo() -> Self {
        Self {
            ligado: AtomicBool::new(false),
            config: Mutex::new(None),
            montados: AtomicU64::new(0),
            enviados: AtomicU64::new(0),
            falhas: AtomicU64::new(0),
        }
    }

    pub fn definir(&self, c: Option<Config>) {
        let ligado = c.is_some();
        *self.config.lock().unwrap_or_else(|p| p.into_inner()) = c.map(Arc::new);
        self.ligado.store(ligado, Relaxed);
    }

    pub fn ligado(&self) -> bool {
        self.ligado.load(Relaxed)
    }

    fn atual(&self) -> Option<Arc<Config>> {
        if !self.ligado() {
            return None;
        }
        self.config
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone()
    }

    /// O ponto de captura do fim de uma execucao (tarefa ou fluxo). O interruptor e a
    /// primeira coisa: desligado, nem o `task.json` se abre. Devolve o envio, para o teste
    /// esperar; quem chama ignora.
    pub fn exportar_tarefa(
        &'static self,
        store: &TaskStore,
        id: &str,
    ) -> Option<tokio::task::JoinHandle<Result<u16, String>>> {
        let cfg = self.atual()?;
        tokio::runtime::Handle::try_current().ok()?;
        let (store, id) = (store.clone(), id.to_string());
        Some(tokio::spawn(async move {
            let c = cfg.clone();
            let corpo = tokio::task::spawn_blocking(move || {
                self.montados.fetch_add(1, Relaxed);
                traco(&store, &id, &c)
            })
            .await
            .map_err(|e| e.to_string())?
            .ok_or("execucao ainda nao terminou (ou sumiu do disco)")?;
            self.enviar(&cfg, "/v1/traces", corpo).await
        }))
    }

    /// As series de `m` agora, como um `ExportMetricsServiceRequest`.
    pub fn exportar_metricas(
        &'static self,
        m: &crate::metricas::Metricas,
    ) -> Option<tokio::task::JoinHandle<Result<u16, String>>> {
        let cfg = self.atual()?;
        let series = m.series()?;
        let corpo = metricas_otlp(&series, &cfg, Utc::now());
        Some(tokio::spawn(async move {
            self.enviar(&cfg, "/v1/metrics", corpo).await
        }))
    }

    async fn enviar(&self, cfg: &Config, caminho: &str, corpo: Value) -> Result<u16, String> {
        // Sem proxy do ambiente: o coletor e servidor do operador, e um HTTP_PROXY herdado
        // mandaria os traces (ids de tarefa, nomes de fluxo) para outro lugar.
        let r = async {
            let cliente = reqwest::Client::builder()
                .no_proxy()
                .timeout(Duration::from_secs(10))
                .build()
                .map_err(|e| e.to_string())?;
            let resp = cliente
                .post(format!("{}{caminho}", cfg.base))
                .header(reqwest::header::CONTENT_TYPE, "application/json")
                .body(corpo.to_string())
                .send()
                .await
                .map_err(|e| format!("coletor {}: {}", cfg.base, e.without_url()))?;
            let st = resp.status().as_u16();
            if resp.status().is_success() {
                Ok(st)
            } else {
                Err(format!("coletor {} respondeu {st}", cfg.base))
            }
        }
        .await;
        match &r {
            Ok(_) => {
                self.enviados.fetch_add(1, Relaxed);
            }
            Err(e) => {
                // A primeira falha e uma a cada cem: coletor fora do ar nao enche o log.
                if self.falhas.fetch_add(1, Relaxed).is_multiple_of(100) {
                    eprintln!("otel: {e}");
                }
            }
        }
        r
    }
}

// ------------------------------------------------------------------ o JSON do OTLP

enum Valor {
    Texto(String),
    Inteiro(i64),
    Real(f64),
    Booleano(bool),
}

/// Um atributo OTLP. A chave fora de `ATRIBUTOS` e erro de quem escreveu o codigo.
fn atributo(chave: &str, v: Valor) -> Value {
    debug_assert!(
        ATRIBUTOS.contains(&chave),
        "atributo fora da lista: {chave}"
    );
    let valor = match v {
        Valor::Texto(s) => json!({"stringValue": s}),
        Valor::Inteiro(n) => json!({"intValue": n.to_string()}),
        Valor::Real(x) => json!({"doubleValue": x}),
        Valor::Booleano(b) => json!({"boolValue": b}),
    };
    json!({"key": chave, "value": valor})
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn sha(t: &str) -> [u8; 32] {
    use sha2::Digest;
    sha2::Sha256::digest(t.as_bytes()).into()
}

/// 16 bytes: o proprio UUID da tarefa de cima (o v7 ja e unico e ordenado no tempo); id
/// que nao e UUID cai no sha256.
fn trace_id(raiz: &str) -> String {
    match uuid::Uuid::parse_str(raiz) {
        Ok(u) if !u.is_nil() => hex(u.as_bytes()),
        _ => hex(&sha(raiz)[..16]),
    }
}

/// 8 bytes, deterministico: reexportar a mesma execucao da os mesmos ids.
fn span_id(chave: &str) -> String {
    let mut b = sha(chave);
    if b[..8].iter().all(|x| *x == 0) {
        b[7] = 1;
    }
    hex(&b[..8])
}

fn nanos(t: DateTime<Utc>) -> String {
    t.timestamp_nanos_opt().unwrap_or(0).max(0).to_string()
}

fn estado(e: TaskStatus) -> &'static str {
    match e {
        TaskStatus::Pending => "pending",
        TaskStatus::AwaitingApproval => "awaiting_approval",
        TaskStatus::AwaitingInput => "awaiting_input",
        TaskStatus::Running => "running",
        TaskStatus::Completed => "completed",
        TaskStatus::Failed => "failed",
        TaskStatus::Cancelled => "cancelled",
        TaskStatus::BudgetExceeded => "budget_exceeded",
    }
}

struct Span {
    span_id: String,
    pai: Option<String>,
    nome: &'static str,
    inicio: DateTime<Utc>,
    fim: DateTime<Utc>,
    atributos: Vec<Value>,
    erro: Option<&'static str>,
    eventos: Vec<Value>,
}

impl Span {
    fn json(self, trace: &str) -> Value {
        let fim = self.fim.max(self.inicio);
        let mut v = json!({
            "traceId": trace,
            "spanId": self.span_id,
            "name": self.nome,
            // SPAN_KIND_INTERNAL: nada aqui e servidor nem cliente de outro servico.
            "kind": 1,
            "startTimeUnixNano": nanos(self.inicio),
            "endTimeUnixNano": nanos(fim),
            "attributes": self.atributos,
            "status": match self.erro {
                // STATUS_CODE_ERROR, com o ESTADO -- nunca o texto do erro.
                Some(e) => json!({"code": 2, "message": e}),
                None => json!({"code": 1}),
            },
        });
        if let Some(p) = self.pai {
            v["parentSpanId"] = json!(p);
        }
        if !self.eventos.is_empty() {
            v["events"] = json!(self.eventos);
        }
        v
    }
}

/// A evidencia da tarefa, na ordem do arquivo (a cadeia de hash e a ordem do tempo).
fn evidencias(store: &TaskStore, id: &str) -> Vec<EvidenceRecord> {
    let Ok(f) = std::fs::File::open(store.evidence_path(id)) else {
        return vec![];
    };
    std::io::BufReader::new(f)
        .lines()
        .map_while(Result::ok)
        .filter_map(|l| serde_json::from_str(&l).ok())
        .take(MAX_SPANS)
        .collect()
}

/// Os milissegundos do UUID v7 (os 48 bits de cima), sem depender do formato do resto.
fn ms_do_v7(id: &str) -> Option<i64> {
    let h: String = id.chars().filter(|c| *c != '-').take(12).collect();
    (h.len() == 12)
        .then(|| i64::from_str_radix(&h, 16).ok())
        .flatten()
}

/// A arvore da execucao: a tarefa de cima e as descendentes (subagente, passo de agente,
/// sub-fluxo). Sem listar a pasta inteira: o id e UUID v7, ordenado no tempo, e a filha
/// nasce depois da mae e antes do fim dela -- so os nomes nessa janela se abrem.
fn arvore(store: &TaskStore, raiz: &Task) -> Vec<Task> {
    let mut v = vec![raiz.clone()];
    let fim = raiz.updated_at.timestamp_millis() + 1_000;
    let mut nomes: Vec<String> = std::fs::read_dir(store.root())
        .map(|d| {
            d.flatten()
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .filter(|n| n.as_str() > raiz.id.as_str() && ms_do_v7(n).is_some_and(|m| m <= fim))
                .collect()
        })
        .unwrap_or_default();
    nomes.sort();
    let mut dentro: BTreeSet<String> = BTreeSet::from([raiz.id.clone()]);
    for n in nomes {
        if v.len() >= MAX_SPANS {
            break;
        }
        if let Ok(t) = store.load(&n)
            && t.parent.as_ref().is_some_and(|p| dentro.contains(p))
        {
            dentro.insert(t.id.clone());
            v.push(t);
        }
    }
    v
}

/// O trace de uma execucao terminada, como um `ExportTraceServiceRequest`; `None` se ela
/// nao terminou ou nao existe.
pub fn traco(store: &TaskStore, id: &str, cfg: &Config) -> Option<Value> {
    let raiz = store.load(id).ok()?;
    if !raiz.status.is_final() {
        return None;
    }
    let trace = trace_id(&raiz.id);
    let tarefas = arvore(store, &raiz);
    // passo de agente do fluxo -> o id do passo, pelo relatorio da mae
    let mut passo_de: BTreeMap<String, String> = BTreeMap::new();
    for t in &tarefas {
        if let Some(r) = relatorio(t) {
            for p in r.passos {
                if let Some(filha) = p.tarefa {
                    passo_de.insert(filha, p.id);
                }
            }
        }
    }
    let mut spans: Vec<Span> = Vec::new();
    for t in &tarefas {
        if spans.len() >= MAX_SPANS {
            break;
        }
        spans_da_tarefa(store, t, &passo_de, &mut spans);
    }
    spans.truncate(MAX_SPANS);
    Some(json!({"resourceSpans": [{
        "resource": cfg.recurso(),
        "scopeSpans": [{
            "scope": {"name": "phxclaw", "version": env!("CARGO_PKG_VERSION")},
            "spans": spans.into_iter().map(|s| s.json(&trace)).collect::<Vec<_>>(),
        }],
    }]}))
}

fn relatorio(t: &Task) -> Option<crate::fluxos::Relatorio> {
    if !t.objective.starts_with(crate::fluxos::PREFIXO_TAREFA) {
        return None;
    }
    serde_json::from_str(t.answer.as_deref()?).ok()
}

fn spans_da_tarefa(
    store: &TaskStore,
    t: &Task,
    passo_de: &BTreeMap<String, String>,
    spans: &mut Vec<Span>,
) {
    let eu = span_id(&format!("{}:tarefa", t.id));
    let fluxo = t.objective.strip_prefix(crate::fluxos::PREFIXO_TAREFA);
    let mut atr = vec![
        atributo("phxclaw.tarefa.id", Valor::Texto(t.id.clone())),
        atributo(
            "phxclaw.tarefa.estado",
            Valor::Texto(estado(t.status).into()),
        ),
        atributo("phxclaw.modelo", Valor::Texto(t.model.clone())),
        atributo(
            "phxclaw.tokens.entrada",
            Valor::Inteiro(t.usage.input_tokens as i64),
        ),
        atributo(
            "phxclaw.tokens.saida",
            Valor::Inteiro(t.usage.output_tokens as i64),
        ),
    ];
    if let Some(nome) = fluxo {
        atr.push(atributo(
            "phxclaw.fluxo.nome",
            Valor::Texto(nome.to_string()),
        ));
    }
    if let Some(p) = passo_de.get(&t.id) {
        atr.push(atributo("phxclaw.passo.id", Valor::Texto(p.clone())));
    }
    if let Some(c) = crate::custo::total(t) {
        atr.push(atributo("phxclaw.custo", Valor::Real(c)));
    }
    // Os passos do fluxo viram eventos do span do fluxo: o relatorio diz o estado de cada
    // um, mas nao a hora -- inventar um intervalo seria mentir no grafico.
    let eventos = relatorio(t)
        .map(|r| {
            r.passos
                .into_iter()
                .map(|p| {
                    json!({
                        "timeUnixNano": nanos(t.updated_at),
                        "name": "phxclaw.passo",
                        "attributes": [
                            atributo("phxclaw.passo.id", Valor::Texto(p.id)),
                            atributo("phxclaw.passo.estado", Valor::Texto(p.estado)),
                            atributo("phxclaw.passo.tentativas", Valor::Inteiro(i64::from(p.tentativas))),
                        ],
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    spans.push(Span {
        span_id: eu.clone(),
        pai: t.parent.as_ref().map(|p| span_id(&format!("{p}:tarefa"))),
        nome: if fluxo.is_some() {
            "phxclaw.fluxo"
        } else {
            "phxclaw.tarefa"
        },
        inicio: t.created_at,
        fim: t.updated_at,
        atributos: atr,
        erro: match t.status {
            TaskStatus::Failed | TaskStatus::BudgetExceeded => Some(estado(t.status)),
            _ => None,
        },
        eventos,
    });
    // As chamadas: os passos da tarefa de objetivo (pensamento = modelo, ferramenta), cada
    // ferramenta casada com o registro de evidencia dela pela ordem; a evidencia que nenhum
    // passo reclamou (o fluxo nao grava passo) vira span proprio.
    let mut ev = evidencias(store, &t.id);
    let mut usados = vec![false; ev.len()];
    let mut antes = t.created_at;
    for s in &t.steps {
        let mut atr = vec![
            atributo("phxclaw.passo.n", Valor::Inteiro(i64::from(s.n))),
            atributo("phxclaw.resultado", Valor::Texto(s.outcome.clone())),
            atributo("phxclaw.inicio_aproximado", Valor::Booleano(true)),
        ];
        let nome = if s.kind == "ferramenta" {
            let ferramenta = s.tool.clone().unwrap_or_default();
            if let Some(i) = (0..ev.len()).find(|i| !usados[*i] && ev[*i].action == ferramenta) {
                usados[i] = true;
                atr.push(atributo(
                    "phxclaw.evidencia.id",
                    Valor::Texto(ev[i].uuid.to_string()),
                ));
                atr.push(atributo(
                    "phxclaw.evidencia.hash",
                    Valor::Texto(ev[i].record_hash.clone()),
                ));
            }
            atr.push(atributo(
                "phxclaw.ferramenta.nome",
                Valor::Texto(ferramenta),
            ));
            "phxclaw.ferramenta"
        } else {
            "phxclaw.modelo"
        };
        spans.push(Span {
            span_id: span_id(&format!("{}:passo:{}", t.id, s.n)),
            pai: Some(eu.clone()),
            nome,
            inicio: antes,
            fim: s.at,
            atributos: atr,
            erro: (s.outcome == "erro").then_some("erro"),
            eventos: vec![],
        });
        antes = s.at;
    }
    let mut antes = t.created_at;
    for (i, r) in ev.drain(..).enumerate() {
        let inicio = antes;
        antes = r.occurred_at;
        if usados[i] {
            continue;
        }
        let resultado = serde_json::to_value(&r.outcome)
            .ok()
            .and_then(|v| v.as_str().map(str::to_string))
            .unwrap_or_default();
        spans.push(Span {
            span_id: span_id(&format!("{}:evidencia:{}", t.id, r.uuid)),
            pai: Some(eu.clone()),
            nome: if r.capability.starts_with("llm.") {
                "phxclaw.modelo"
            } else {
                "phxclaw.ferramenta"
            },
            inicio,
            fim: r.occurred_at,
            erro: matches!(resultado.as_str(), "failed" | "denied").then_some("erro"),
            atributos: vec![
                atributo("phxclaw.ferramenta.nome", Valor::Texto(r.action.clone())),
                atributo("phxclaw.resultado", Valor::Texto(resultado)),
                atributo("phxclaw.evidencia.id", Valor::Texto(r.uuid.to_string())),
                atributo(
                    "phxclaw.evidencia.hash",
                    Valor::Texto(r.record_hash.clone()),
                ),
                atributo("phxclaw.inicio_aproximado", Valor::Booleano(true)),
            ],
            eventos: vec![],
        });
    }
}

/// As series como um `ExportMetricsServiceRequest`, temporalidade CUMULATIVA (o contador e
/// do processo e so cresce, como no Prometheus). O histograma do Prometheus guarda as
/// contagens ACUMULADAS por faixa; o OTLP quer a contagem DE CADA faixa (`bucketCounts`,
/// com uma a mais que `explicitBounds`, a do +Inf).
pub fn metricas_otlp(series: &[Serie], cfg: &Config, agora: DateTime<Utc>) -> Value {
    let inicio = nanos(inicio_do_processo());
    let fim = nanos(agora);
    let metricas: Vec<Value> = series
        .iter()
        .map(|s| {
            let corpo = match &s.tipo {
                TipoDeSerie::Contador(pontos) => json!({"sum": {
                    "aggregationTemporality": 2,
                    "isMonotonic": true,
                    "dataPoints": pontos.iter().map(|(r, v)| {
                        let mut p = json!({
                            "startTimeUnixNano": inicio,
                            "timeUnixNano": fim,
                            "attributes": r.map(|(k, v)| vec![atributo(k, Valor::Texto(v.into()))]).unwrap_or_default(),
                        });
                        match v {
                            Numero::Inteiro(x) => p["asInt"] = json!(x.to_string()),
                            Numero::Dinheiro(x) => p["asDouble"] = json!(x),
                        }
                        p
                    }).collect::<Vec<_>>(),
                }}),
                TipoDeSerie::Histograma {
                    faixas,
                    acumuladas,
                    total,
                    soma,
                } => {
                    let mut por_faixa: Vec<String> = Vec::with_capacity(acumuladas.len() + 1);
                    let mut anterior = 0u64;
                    for a in acumuladas {
                        por_faixa.push(a.saturating_sub(anterior).to_string());
                        anterior = *a;
                    }
                    por_faixa.push(total.saturating_sub(anterior).to_string());
                    json!({"histogram": {
                        "aggregationTemporality": 2,
                        "dataPoints": [{
                            "startTimeUnixNano": inicio,
                            "timeUnixNano": fim,
                            "count": total.to_string(),
                            "sum": soma,
                            "bucketCounts": por_faixa,
                            "explicitBounds": faixas,
                        }],
                    }})
                }
            };
            let mut m = json!({"name": s.nome, "description": s.ajuda});
            if let (Some(o), Some(c)) = (m.as_object_mut(), corpo.as_object()) {
                o.extend(c.clone());
            }
            if s.nome.ends_with("_segundos") {
                m["unit"] = json!("s");
            }
            m
        })
        .collect();
    json!({"resourceMetrics": [{
        "resource": cfg.recurso(),
        "scopeMetrics": [{
            "scope": {"name": "phxclaw", "version": env!("CARGO_PKG_VERSION")},
            "metrics": metricas,
        }],
    }]})
}

fn inicio_do_processo() -> DateTime<Utc> {
    static INICIO: std::sync::OnceLock<DateTime<Utc>> = std::sync::OnceLock::new();
    *INICIO.get_or_init(Utc::now)
}
