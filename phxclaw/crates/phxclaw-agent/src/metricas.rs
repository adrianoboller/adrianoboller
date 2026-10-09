//! `GET /metrics` no formato de exposicao de texto do Prometheus: tarefas e execucoes de
//! fluxo por estado, passos, chamadas de ferramenta e de modelo, tokens e latencia em
//! histograma.
//!
//! Decisoes que valem saber:
//! - **Desligada custa zero, e o interruptor vem ANTES do trabalho.** `medindo` devolve o
//!   MESMO agente quando desligada (nenhum involucro, nenhum relogio por chamada), e
//!   `fim_de_tarefa` recebe um fecho que so roda depois de conferir o interruptor. O padrao
//!   e desligado, como o `N8N_METRICS` do n8n: ligar e `api.metricas` no config.json.
//! - **Rotulo so de conjunto fechado** (`estado`, `resultado`, `tipo`): nada de nome de
//!   ferramenta, argumento, objetivo ou credencial. Nome de ferramenta de MCP e de plugin
//!   vem de fora e poderia carregar qualquer coisa; e cardinalidade sem teto no coletor.
//! - **Atras do mesmo portao das outras rotas** (papel leitor na matriz do `rbac.rs`).
//! - Contadores do PROCESSO: reiniciar zera, que e o que o Prometheus espera de um
//!   `counter` (ele trata o recomeco).
//! - **Custo por resultado** (R2): o dinheiro gasto pelas tarefas e pelos fluxos, pelo
//!   estado em que pararam, na moeda da tabela de precos (`custo.precos`). A moeda NAO e
//!   rotulo (e texto do operador); quem le a metrica le a moeda na tabela. Tarefa sem custo
//!   medido nao soma zero: conta em `*_custo_nao_medido_total`.
//! - **A completacao do editor conta a parte** (`phxclaw_ide_completar_*`): ela gasta o
//!   modelo fora de qualquer tarefa, e portanto fora de qualquer orcamento; o teto dela e
//!   o `ide.ia_teto_tokens_hora` (`ide.rs`), e as recusas por ele contam aqui.

use crate::motor::Agent;
use crate::tarefa::{Task, TaskStatus};
use phxclaw_agent_core::{
    BoxFut, Llm, LlmError, LlmOptions, LlmReply, Message, Tool, ToolContext, ToolError, ToolOutput,
    ToolSpec,
};
use serde_json::Value;
use std::fmt::Write as _;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering::Relaxed};
use std::time::{Duration, Instant};

/// Os estados de `TaskStatus`, na ordem do indice de `indice_do_estado` (o nome e o do fio).
const ESTADOS: [&str; N_ESTADOS] = [
    "pending",
    "awaiting_approval",
    "awaiting_input",
    "running",
    "completed",
    "failed",
    "cancelled",
    "budget_exceeded",
];

const N_ESTADOS: usize = 8;

fn indice_do_estado(e: TaskStatus) -> usize {
    // `match` exaustivo: estado novo nao compila sem lugar aqui.
    match e {
        TaskStatus::Pending => 0,
        TaskStatus::AwaitingApproval => 1,
        TaskStatus::AwaitingInput => 2,
        TaskStatus::Running => 3,
        TaskStatus::Completed => 4,
        TaskStatus::Failed => 5,
        TaskStatus::Cancelled => 6,
        TaskStatus::BudgetExceeded => 7,
    }
}

const MAX_FAIXAS: usize = 16;
/// Ferramenta: de milissegundos (calculadora) a minutos (shell, navegador).
const FAIXAS_FERRAMENTA: &[f64] = &[
    0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0, 30.0, 60.0, 120.0, 300.0,
];
/// Tarefa inteira: de segundos a uma hora.
const FAIXAS_TAREFA: &[f64] = &[
    1.0, 5.0, 10.0, 30.0, 60.0, 120.0, 300.0, 600.0, 1800.0, 3600.0,
];

/// Histograma de faixas fixas, sem trava: cada observacao e um incremento atomico.
pub struct Histograma {
    faixas: &'static [f64],
    contagens: [AtomicU64; MAX_FAIXAS],
    soma_us: AtomicU64,
    total: AtomicU64,
}

impl Histograma {
    const fn novo(faixas: &'static [f64]) -> Self {
        Self {
            faixas,
            contagens: [const { AtomicU64::new(0) }; MAX_FAIXAS],
            soma_us: AtomicU64::new(0),
            total: AtomicU64::new(0),
        }
    }

    fn observar(&self, d: Duration) {
        let s = d.as_secs_f64();
        if let Some(i) = self.faixas.iter().position(|f| s <= *f) {
            self.contagens[i].fetch_add(1, Relaxed);
        }
        self.soma_us
            .fetch_add(u64::try_from(d.as_micros()).unwrap_or(u64::MAX), Relaxed);
        self.total.fetch_add(1, Relaxed);
    }

    fn serie(&self, nome: &'static str, ajuda: &'static str) -> Serie {
        let mut acumulado = 0u64;
        let acumuladas = self.contagens[..self.faixas.len()]
            .iter()
            .map(|c| {
                acumulado += c.load(Relaxed);
                acumulado
            })
            .collect();
        Serie {
            nome,
            ajuda,
            tipo: TipoDeSerie::Histograma {
                faixas: self.faixas,
                acumuladas,
                total: self.total.load(Relaxed),
                soma: self.soma_us.load(Relaxed) as f64 / 1e6,
            },
        }
    }
}

/// O que se conta no fim de uma tarefa de objetivo. Montado por quem chama SO quando a
/// metrica esta ligada (vem num fecho).
pub struct FimDeTarefa {
    pub estado: TaskStatus,
    pub passos: u64,
    pub duracao: Option<Duration>,
    /// Custo da tarefa com as filhas, na moeda da tabela; `None` = nao medido.
    pub custo: Option<f64>,
}

impl FimDeTarefa {
    pub fn de(t: &Task, inicio: Option<Instant>) -> Self {
        Self {
            estado: t.status,
            passos: t.steps.len() as u64,
            duracao: inicio.map(|i| i.elapsed()),
            custo: crate::custo::total(t),
        }
    }
}

/// O que se conta no fim de uma execucao de fluxo: o estado e o custo dos passos.
pub struct FimDeFluxo {
    pub estado: TaskStatus,
    pub custo: Option<f64>,
}

impl FimDeFluxo {
    pub fn de(t: &Task) -> Self {
        Self {
            estado: t.status,
            custo: crate::custo::total(t),
        }
    }
}

/// Dinheiro em milionesimos: soma atomica sem trava, e a sexta casa e a do relatorio.
fn micro(v: f64) -> u64 {
    (v * 1e6).round().clamp(0.0, u64::MAX as f64) as u64
}

pub struct Metricas {
    ligada: AtomicBool,
    tarefas: [AtomicU64; N_ESTADOS],
    fluxos: [AtomicU64; N_ESTADOS],
    tarefa_custo_micro: [AtomicU64; N_ESTADOS],
    tarefa_custo_nao_medido: [AtomicU64; N_ESTADOS],
    fluxo_custo_micro: [AtomicU64; N_ESTADOS],
    fluxo_custo_nao_medido: [AtomicU64; N_ESTADOS],
    passos: AtomicU64,
    ferramenta_ok: AtomicU64,
    ferramenta_erro: AtomicU64,
    ferramenta_duracao: Histograma,
    tarefa_duracao: Histograma,
    modelo_chamadas: AtomicU64,
    modelo_erros: AtomicU64,
    tokens_entrada: AtomicU64,
    tokens_saida: AtomicU64,
    ide_completar_ok: AtomicU64,
    ide_completar_recusadas: AtomicU64,
    ide_completar_tokens: AtomicU64,
    ide_completar_custo_micro: AtomicU64,
    ide_completar_custo_nao_medido: AtomicU64,
}

/// As metricas do processo do `servir`.
pub static GLOBAL: Metricas = Metricas::nova();

/// A rota `GET /metrics` responde? Separada da contagem: o exportador OpenTelemetry
/// (`otel.rs`) conta sem abrir uma rota que o operador nao pediu.
static EXPOSTA: AtomicBool = AtomicBool::new(false);

/// Liga ou desliga as do processo (o `servir`, pela chave `api.metricas`): conta e expoe.
pub fn ligar(sim: bool) {
    GLOBAL.ligada.store(sim, Relaxed);
    EXPOSTA.store(sim, Relaxed);
}

/// Conta sem expor: o `otel.url` precisa das series, nao da rota.
pub fn contar_para_exportar() {
    GLOBAL.ligada.store(true, Relaxed);
}

impl Default for Metricas {
    fn default() -> Self {
        Self::nova()
    }
}

impl Metricas {
    pub const fn nova() -> Self {
        Self {
            ligada: AtomicBool::new(false),
            tarefas: [const { AtomicU64::new(0) }; N_ESTADOS],
            fluxos: [const { AtomicU64::new(0) }; N_ESTADOS],
            tarefa_custo_micro: [const { AtomicU64::new(0) }; N_ESTADOS],
            tarefa_custo_nao_medido: [const { AtomicU64::new(0) }; N_ESTADOS],
            fluxo_custo_micro: [const { AtomicU64::new(0) }; N_ESTADOS],
            fluxo_custo_nao_medido: [const { AtomicU64::new(0) }; N_ESTADOS],
            passos: AtomicU64::new(0),
            ferramenta_ok: AtomicU64::new(0),
            ferramenta_erro: AtomicU64::new(0),
            ferramenta_duracao: Histograma::novo(FAIXAS_FERRAMENTA),
            tarefa_duracao: Histograma::novo(FAIXAS_TAREFA),
            modelo_chamadas: AtomicU64::new(0),
            modelo_erros: AtomicU64::new(0),
            tokens_entrada: AtomicU64::new(0),
            tokens_saida: AtomicU64::new(0),
            ide_completar_ok: AtomicU64::new(0),
            ide_completar_recusadas: AtomicU64::new(0),
            ide_completar_tokens: AtomicU64::new(0),
            ide_completar_custo_micro: AtomicU64::new(0),
            ide_completar_custo_nao_medido: AtomicU64::new(0),
        }
    }

    pub fn ligada(&self) -> bool {
        self.ligada.load(Relaxed)
    }

    pub fn definir(&self, sim: bool) {
        self.ligada.store(sim, Relaxed);
    }

    /// O agente com modelo e ferramentas contados -- ou o MESMO agente, intocado, quando
    /// desligada: o interruptor e a primeira coisa, antes de qualquer involucro.
    pub fn medindo(&'static self, mut a: Agent) -> Agent {
        if !self.ligada() {
            return a;
        }
        a.llm = Arc::new(LlmMedido {
            interno: a.llm,
            m: self,
        });
        a.tools = a
            .tools
            .into_iter()
            .map(|t| {
                Arc::new(ToolMedida {
                    interno: t,
                    m: self,
                }) as Arc<dyn Tool>
            })
            .collect();
        a
    }

    /// Fim de uma tarefa de objetivo. O fecho so roda com a metrica ligada.
    pub fn fim_de_tarefa(&self, fim: impl FnOnce() -> FimDeTarefa) {
        if !self.ligada() {
            return;
        }
        let f = fim();
        let i = indice_do_estado(f.estado);
        self.tarefas[i].fetch_add(1, Relaxed);
        match f.custo {
            Some(c) => self.tarefa_custo_micro[i].fetch_add(micro(c), Relaxed),
            None => self.tarefa_custo_nao_medido[i].fetch_add(1, Relaxed),
        };
        self.passos.fetch_add(f.passos, Relaxed);
        if let Some(d) = f.duracao {
            self.tarefa_duracao.observar(d);
        }
    }

    /// Fim de um trecho de execucao de fluxo (do disparo ou da retomada ate parar), pelo
    /// estado em que parou. O fecho so roda com a metrica ligada.
    pub fn fim_de_fluxo(&self, fim: impl FnOnce() -> Option<FimDeFluxo>) {
        if !self.ligada() {
            return;
        }
        if let Some(f) = fim() {
            let i = indice_do_estado(f.estado);
            self.fluxos[i].fetch_add(1, Relaxed);
            match f.custo {
                Some(c) => self.fluxo_custo_micro[i].fetch_add(micro(c), Relaxed),
                None => self.fluxo_custo_nao_medido[i].fetch_add(1, Relaxed),
            };
        }
    }

    /// Uma completacao do editor que chamou o modelo: os tokens e o custo (ausente: sem
    /// preco, conta em `nao_medido`). Desligada, nada -- nem o fecho do preco roda.
    pub fn completacao(&self, tokens: u64, custo: impl FnOnce() -> Option<f64>) {
        if !self.ligada() {
            return;
        }
        let custo = custo();
        self.ide_completar_ok.fetch_add(1, Relaxed);
        self.ide_completar_tokens.fetch_add(tokens, Relaxed);
        match custo {
            Some(c) => self.ide_completar_custo_micro.fetch_add(micro(c), Relaxed),
            None => self.ide_completar_custo_nao_medido.fetch_add(1, Relaxed),
        };
    }

    /// Uma completacao recusada pelo teto `ide.ia_teto_tokens_hora`.
    pub fn completacao_recusada(&self) {
        if self.ligada() {
            self.ide_completar_recusadas.fetch_add(1, Relaxed);
        }
    }

    /// As series, uma vez: o texto do Prometheus (`texto`) e o OTLP (`otel.rs`) saem daqui,
    /// e por isso uma metrica nova aparece nos dois ou em nenhum. `None` desligada.
    pub fn series(&self) -> Option<Vec<Serie>> {
        if !self.ligada() {
            return None;
        }
        let por_estado = |nome, ajuda, v: &[AtomicU64; N_ESTADOS], dinheiro: bool| Serie {
            nome,
            ajuda,
            tipo: TipoDeSerie::Contador(
                ESTADOS
                    .iter()
                    .enumerate()
                    .map(|(i, e)| {
                        let x = v[i].load(Relaxed);
                        (
                            Some(("estado", *e)),
                            if dinheiro {
                                Numero::Dinheiro(x as f64 / 1e6)
                            } else {
                                Numero::Inteiro(x)
                            },
                        )
                    })
                    .collect(),
            ),
        };
        let contador =
            |nome, ajuda, pontos: Vec<(Option<(&'static str, &'static str)>, u64)>| Serie {
                nome,
                ajuda,
                tipo: TipoDeSerie::Contador(
                    pontos
                        .into_iter()
                        .map(|(r, v)| (r, Numero::Inteiro(v)))
                        .collect(),
                ),
            };
        Some(vec![
            por_estado(
                "phxclaw_tarefas_total",
                "Tarefas de objetivo terminadas, pelo estado final.",
                &self.tarefas,
                false,
            ),
            por_estado(
                "phxclaw_fluxo_execucoes_total",
                "Execucoes de fluxo (disparo ou retomada) pelo estado em que pararam.",
                &self.fluxos,
                false,
            ),
            por_estado(
                "phxclaw_tarefa_custo_total",
                "Custo das tarefas de objetivo (com as filhas), pelo estado final, na moeda da tabela PHXCLAW_CUSTO_PRECOS.",
                &self.tarefa_custo_micro,
                true,
            ),
            por_estado(
                "phxclaw_tarefa_custo_nao_medido_total",
                "Tarefas de objetivo terminadas sem custo medido (modelo fora da tabela, ou sem tabela), pelo estado final.",
                &self.tarefa_custo_nao_medido,
                false,
            ),
            por_estado(
                "phxclaw_fluxo_custo_total",
                "Custo das execucoes de fluxo (todos os passos), pelo estado em que pararam, na moeda da tabela PHXCLAW_CUSTO_PRECOS.",
                &self.fluxo_custo_micro,
                true,
            ),
            por_estado(
                "phxclaw_fluxo_custo_nao_medido_total",
                "Execucoes de fluxo paradas sem custo medido, pelo estado em que pararam.",
                &self.fluxo_custo_nao_medido,
                false,
            ),
            contador(
                "phxclaw_passos_total",
                "Passos gravados pelas tarefas de objetivo terminadas.",
                vec![(None, self.passos.load(Relaxed))],
            ),
            contador(
                "phxclaw_ferramenta_chamadas_total",
                "Chamadas de ferramenta, pelo resultado.",
                vec![
                    (Some(("resultado", "ok")), self.ferramenta_ok.load(Relaxed)),
                    (
                        Some(("resultado", "erro")),
                        self.ferramenta_erro.load(Relaxed),
                    ),
                ],
            ),
            contador(
                "phxclaw_modelo_chamadas_total",
                "Pedidos ao modelo, pelo resultado.",
                vec![
                    (
                        Some(("resultado", "ok")),
                        self.modelo_chamadas.load(Relaxed),
                    ),
                    (Some(("resultado", "erro")), self.modelo_erros.load(Relaxed)),
                ],
            ),
            contador(
                "phxclaw_tokens_total",
                "Tokens informados pelo provedor.",
                vec![
                    (Some(("tipo", "entrada")), self.tokens_entrada.load(Relaxed)),
                    (Some(("tipo", "saida")), self.tokens_saida.load(Relaxed)),
                ],
            ),
            contador(
                "phxclaw_ide_completar_total",
                "Completacoes por IA do editor (fora de qualquer tarefa e orcamento), pelo resultado; recusada = teto ide.ia_teto_tokens_hora.",
                vec![
                    (
                        Some(("resultado", "ok")),
                        self.ide_completar_ok.load(Relaxed),
                    ),
                    (
                        Some(("resultado", "recusada")),
                        self.ide_completar_recusadas.load(Relaxed),
                    ),
                ],
            ),
            contador(
                "phxclaw_ide_completar_custo_nao_medido_total",
                "Completacoes do editor sem custo medido (modelo fora da tabela, ou sem tabela).",
                vec![(None, self.ide_completar_custo_nao_medido.load(Relaxed))],
            ),
            contador(
                "phxclaw_ide_completar_tokens_total",
                "Tokens (entrada + saida) gastos pela completacao por IA do editor.",
                vec![(None, self.ide_completar_tokens.load(Relaxed))],
            ),
            Serie {
                nome: "phxclaw_ide_completar_custo_total",
                ajuda: "Custo da completacao por IA do editor, na moeda da tabela PHXCLAW_CUSTO_PRECOS.",
                tipo: TipoDeSerie::Contador(vec![(
                    None,
                    Numero::Dinheiro(self.ide_completar_custo_micro.load(Relaxed) as f64 / 1e6),
                )]),
            },
            self.ferramenta_duracao.serie(
                "phxclaw_ferramenta_duracao_segundos",
                "Latencia de cada chamada de ferramenta.",
            ),
            self.tarefa_duracao.serie(
                "phxclaw_tarefa_duracao_segundos",
                "Duracao da execucao de cada tarefa de objetivo.",
            ),
        ])
    }

    /// O texto de exposicao, ou `None` desligada (a rota responde 404).
    pub fn texto(&self) -> Option<String> {
        let mut o = String::new();
        for s in self.series()? {
            let _ = writeln!(o, "# HELP {} {}", s.nome, s.ajuda);
            match &s.tipo {
                TipoDeSerie::Contador(pontos) => {
                    let _ = writeln!(o, "# TYPE {} counter", s.nome);
                    for (r, v) in pontos {
                        let rotulo = r
                            .map(|(k, v)| format!("{{{k}=\"{v}\"}}"))
                            .unwrap_or_default();
                        let _ = match v {
                            Numero::Inteiro(x) => writeln!(o, "{}{rotulo} {x}", s.nome),
                            Numero::Dinheiro(x) => writeln!(o, "{}{rotulo} {x:.6}", s.nome),
                        };
                    }
                }
                TipoDeSerie::Histograma {
                    faixas,
                    acumuladas,
                    total,
                    soma,
                } => {
                    let _ = writeln!(o, "# TYPE {} histogram", s.nome);
                    for (f, n) in faixas.iter().zip(acumuladas) {
                        let _ = writeln!(o, "{}_bucket{{le=\"{f}\"}} {n}", s.nome);
                    }
                    let _ = writeln!(o, "{}_bucket{{le=\"+Inf\"}} {total}", s.nome);
                    let _ = writeln!(o, "{}_sum {soma}\n{}_count {total}", s.nome, s.nome);
                }
            }
        }
        Some(o)
    }
}

/// Um numero de serie: contagem, ou dinheiro (seis casas, a do relatorio).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Numero {
    Inteiro(u64),
    Dinheiro(f64),
}

#[derive(Debug, Clone, PartialEq)]
pub enum TipoDeSerie {
    /// Pontos de um contador; o rotulo, quando ha, e de conjunto fechado.
    Contador(Vec<(Option<(&'static str, &'static str)>, Numero)>),
    /// Histograma de faixas fixas, com as contagens ACUMULADAS do Prometheus.
    Histograma {
        faixas: &'static [f64],
        acumuladas: Vec<u64>,
        total: u64,
        soma: f64,
    },
}

/// Uma serie das metricas do processo.
#[derive(Debug, Clone, PartialEq)]
pub struct Serie {
    pub nome: &'static str,
    pub ajuda: &'static str,
    pub tipo: TipoDeSerie,
}

struct LlmMedido {
    interno: Arc<dyn Llm>,
    m: &'static Metricas,
}

impl Llm for LlmMedido {
    fn id(&self) -> String {
        self.interno.id()
    }
    fn provedores(&self) -> Vec<String> {
        self.interno.provedores()
    }
    fn chat<'a>(
        &'a self,
        messages: &'a [Message],
        tools: &'a [ToolSpec],
        options: &'a LlmOptions,
    ) -> BoxFut<'a, Result<LlmReply, LlmError>> {
        Box::pin(async move {
            let r = self.interno.chat(messages, tools, options).await;
            match &r {
                Ok(resp) => {
                    self.m.modelo_chamadas.fetch_add(1, Relaxed);
                    self.m
                        .tokens_entrada
                        .fetch_add(resp.usage.input_tokens, Relaxed);
                    self.m
                        .tokens_saida
                        .fetch_add(resp.usage.output_tokens, Relaxed);
                }
                Err(_) => {
                    self.m.modelo_erros.fetch_add(1, Relaxed);
                }
            }
            r
        })
    }
}

struct ToolMedida {
    interno: Arc<dyn Tool>,
    m: &'static Metricas,
}

// Repassa TODO metodo do trait: um que caisse no padrao mudaria o comportamento da
// ferramenta medida (o `comando_de_shell` e o que as regras de comando conferem).
impl Tool for ToolMedida {
    fn spec(&self) -> ToolSpec {
        self.interno.spec()
    }
    fn capability(&self) -> &'static str {
        self.interno.capability()
    }
    fn run<'a>(
        &'a self,
        args: Value,
        ctx: &'a ToolContext,
    ) -> BoxFut<'a, Result<ToolOutput, ToolError>> {
        Box::pin(async move {
            let t0 = Instant::now();
            let r = self.interno.run(args, ctx).await;
            self.m.ferramenta_duracao.observar(t0.elapsed());
            match &r {
                Ok(_) => self.m.ferramenta_ok.fetch_add(1, Relaxed),
                Err(_) => self.m.ferramenta_erro.fetch_add(1, Relaxed),
            };
            r
        })
    }
    fn finish<'a>(&'a self, task_id: &'a str) -> BoxFut<'a, ()> {
        self.interno.finish(task_id)
    }
    fn comando_de_shell(&self, args: &Value) -> Option<String> {
        self.interno.comando_de_shell(args)
    }
}

/// `GET /metrics`: o Bearer primeiro (o portao, com usuarios, ja decidiu o papel), depois
/// o interruptor -- desligada e 404, para o coletor dizer «alvo sem metrica» e nao «zero».
pub async fn expor(
    axum::extract::State(s): axum::extract::State<crate::api::ApiState>,
    h: axum::http::HeaderMap,
) -> axum::response::Response {
    use axum::http::{StatusCode, header};
    use axum::response::IntoResponse;
    if let Err(e) = crate::api::auth(&s, &h) {
        return e.into_response();
    }
    // A rota fechada responde antes de montar texto nenhum.
    let texto = if EXPOSTA.load(Relaxed) {
        GLOBAL.texto()
    } else {
        None
    };
    match texto {
        Some(t) => (
            [(
                header::CONTENT_TYPE,
                "text/plain; version=0.0.4; charset=utf-8",
            )],
            t,
        )
            .into_response(),
        None => (StatusCode::NOT_FOUND, "metricas desligadas (api.metricas)").into_response(),
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::tarefa::TaskStore;
    use serde_json::json;

    struct Eco;
    impl Tool for Eco {
        fn spec(&self) -> ToolSpec {
            ToolSpec {
                name: "eco".into(),
                description: String::new(),
                parameters: json!({"type": "object"}),
            }
        }
        fn capability(&self) -> &'static str {
            "calc"
        }
        fn run<'a>(
            &'a self,
            args: Value,
            _ctx: &'a ToolContext,
        ) -> BoxFut<'a, Result<ToolOutput, ToolError>> {
            Box::pin(async move { Ok(ToolOutput::text(args.to_string())) })
        }
    }

    struct Mudo;
    impl Llm for Mudo {
        fn id(&self) -> String {
            "mudo".into()
        }
        fn chat<'a>(
            &'a self,
            _m: &'a [Message],
            _t: &'a [ToolSpec],
            _o: &'a LlmOptions,
        ) -> BoxFut<'a, Result<LlmReply, LlmError>> {
            Box::pin(async { Err(LlmError::Parse("mudo".into())) })
        }
    }

    fn agente() -> Agent {
        Agent::new(
            Arc::new(Mudo),
            vec![Arc::new(Eco)],
            Default::default(),
            TaskStore::new(std::env::temp_dir().join("phx-metricas-store")).unwrap(),
        )
    }

    fn nova() -> &'static Metricas {
        Box::leak(Box::new(Metricas::nova()))
    }

    #[test]
    fn desligada_nao_envolve_o_agente_nem_roda_o_fecho() {
        let m = nova();
        let a = agente();
        let (llm, tool) = (a.llm.clone(), a.tools[0].clone());
        let b = m.medindo(a);
        assert!(Arc::ptr_eq(&llm, &b.llm), "envolveu o modelo desligada");
        assert!(
            Arc::ptr_eq(&tool, &b.tools[0]),
            "envolveu a ferramenta desligada"
        );
        let mut rodou = false;
        m.fim_de_tarefa(|| {
            rodou = true;
            FimDeTarefa {
                estado: TaskStatus::Completed,
                passos: 1,
                duracao: None,
                custo: None,
            }
        });
        m.fim_de_fluxo(|| {
            rodou = true;
            None
        });
        assert!(!rodou, "trabalho feito antes do interruptor");
        assert!(m.texto().is_none());
    }

    /// B6: a completacao do editor gasta fora de qualquer tarefa; conta a parte.
    #[test]
    fn a_completacao_do_editor_conta_a_parte_e_desligada_nao_roda_o_fecho() {
        let m = nova();
        m.completacao(100, || panic!("desligada nao calcula o preco"));
        m.definir(true);
        m.completacao(100, || Some(0.5));
        m.completacao(20, || None);
        m.completacao_recusada();
        let t = m.texto().unwrap();
        for l in [
            "phxclaw_ide_completar_total{resultado=\"ok\"} 2",
            "phxclaw_ide_completar_total{resultado=\"recusada\"} 1",
            "phxclaw_ide_completar_tokens_total 120",
            "phxclaw_ide_completar_custo_total 0.500000",
            "phxclaw_ide_completar_custo_nao_medido_total 1",
        ] {
            assert!(t.contains(l), "falta {l}:\n{t}");
        }
    }

    #[tokio::test]
    async fn ligada_conta_e_os_rotulos_sao_de_conjunto_fechado() {
        let m = nova();
        m.definir(true);
        let a = m.medindo(agente());
        let ctx = ToolContext {
            task_id: "t".into(),
            workdir: std::env::temp_dir(),
            timeout: Duration::from_secs(1),
        };
        let segredo = "ghp_SEGREDO_QUE_NAO_PODE_SAIR";
        a.tools[0]
            .run(json!({"token": segredo}), &ctx)
            .await
            .unwrap();
        let _ = a.llm.chat(&[], &[], &LlmOptions::default()).await;
        m.fim_de_tarefa(|| FimDeTarefa {
            estado: TaskStatus::Failed,
            passos: 3,
            duracao: Some(Duration::from_millis(1500)),
            custo: None,
        });
        m.fim_de_fluxo(|| {
            Some(FimDeFluxo {
                estado: TaskStatus::AwaitingInput,
                custo: Some(0.25),
            })
        });
        let t = m.texto().unwrap();
        assert!(
            t.contains("phxclaw_ferramenta_chamadas_total{resultado=\"ok\"} 1"),
            "{t}"
        );
        assert!(
            t.contains("phxclaw_modelo_chamadas_total{resultado=\"erro\"} 1"),
            "{t}"
        );
        assert!(
            t.contains("phxclaw_tarefas_total{estado=\"failed\"} 1"),
            "{t}"
        );
        assert!(t.contains("phxclaw_passos_total 3"), "{t}");
        assert!(
            t.contains("phxclaw_fluxo_execucoes_total{estado=\"awaiting_input\"} 1"),
            "{t}"
        );
        assert!(
            t.contains("phxclaw_tarefa_duracao_segundos_bucket{le=\"5\"} 1"),
            "{t}"
        );
        assert!(
            t.contains("phxclaw_tarefa_duracao_segundos_bucket{le=\"1\"} 0"),
            "{t}"
        );
        assert!(
            t.contains("phxclaw_ferramenta_duracao_segundos_count 1"),
            "{t}"
        );
        // Custo por resultado: o nao medido conta como nao medido, nunca como zero somado.
        assert!(
            t.contains("phxclaw_tarefa_custo_nao_medido_total{estado=\"failed\"} 1"),
            "{t}"
        );
        assert!(
            t.contains("phxclaw_fluxo_custo_total{estado=\"awaiting_input\"} 0.250000"),
            "{t}"
        );
        assert!(!t.contains(segredo) && !t.contains("eco"), "{t}");
        // Todo rotulo publicado e de conjunto fechado: um rotulo novo com valor livre
        // (nome de ferramenta, argumento) reprova aqui antes de chegar a um coletor.
        let permitidos: Vec<String> = ESTADOS
            .iter()
            .map(|e| format!("estado=\"{e}\""))
            .chain(
                [
                    "resultado=\"ok\"",
                    "resultado=\"erro\"",
                    "resultado=\"recusada\"",
                    "tipo=\"entrada\"",
                    "tipo=\"saida\"",
                ]
                .map(String::from),
            )
            .collect();
        for linha in t.lines().filter(|l| !l.starts_with('#')) {
            if let (Some(i), Some(f)) = (linha.find('{'), linha.find('}')) {
                let r = &linha[i + 1..f];
                assert!(
                    r.starts_with("le=\"") || permitidos.iter().any(|p| p == r),
                    "rotulo fora do conjunto fechado: {linha}"
                );
            }
        }
    }
}
