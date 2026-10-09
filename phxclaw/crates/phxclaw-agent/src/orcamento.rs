//! Orcamento por tarefa e por fluxo, em tokens e em dinheiro (R3 do radar de 09/10/2026).
//!
//! Tres camadas, todas da configuracao do operador (`orcamento.*`), e o pedido da API:
//! - o **padrao** da tarefa e o do fluxo, que vale quando o pedido nao traz o seu;
//! - o **teto global**, que nenhum pedido ultrapassa: acima dele a criacao e RECUSADA
//!   dizendo o teto, em vez de cortar calado -- quem pediu 10 e recebeu 5 sem saber
//!   planejaria com o 10;
//! - e o motor, que confere de novo e corta no teto mesmo para o caminho que nao passou
//!   pela API (CLI, gatilho, agenda): a guarda que so existe na porta e porta dos fundos
//!   para quem entra pela janela.
//!
//! O gasto que conta e o da tarefa SOMADO ao das descendentes (subagente, passo de fluxo,
//! sub-fluxo): sem isso, um orcamento de tarefa seria contornado abrindo filhas. A soma
//! mora numa conta em memoria por tarefa em execucao, encadeada pelo `parent`; cada
//! chamada ao modelo cobra a propria conta e as dos ancestrais, e a primeira que bater
//! para quem cobrou. O ancestral para na proxima conferencia dele.
//!
//! Ao bater, a tarefa PARA em `budget_exceeded` dizendo quanto gastou e qual teto bateu;
//! nunca segue calada. O custo so se sabe com a resposta, entao a conta e conferida ANTES
//! de cada chamada e cobrada DEPOIS dela, e a mensagem diz o numero real, nao o teto.
//!
//! **Toda chamada ao modelo sob uma tarefa cobra a conta dela, nao so a do laco do motor.**
//! Ferramenta que chama o modelo por dentro (pesquisa profunda, revisao de codigo) e o
//! classificador da rota gastavam fora da conta -- a pesquisa profunda sozinha, ~9 mil
//! tokens por uso. O caminho unico e o `LlmDaTarefa`: a montagem envolve o modelo UMA vez
//! (o mesmo `Arc` vai ao agente, as ferramentas e aos subagentes), e o motor marca a conta
//! da tarefa na tarefa do tokio (`ESCOPO`) durante a propria chamada e durante cada
//! ferramenta. O envolto confere antes, cobra depois e, ao bater, devolve erro: a
//! ferramenta falha e o motor para em `budget_exceeded` logo depois dela. A chamada do
//! proprio laco continua cobrada pelo motor (ele sabe, pelo diario da rota, quem atendeu),
//! e o envolto a deixa passar sem cobrar de novo.
//!
//! **Excesso maximo, com filhas em paralelo: uma chamada.** Antes, cada filha conferia a
//! conta da mae sem ver as irmas em voo, e `max_parallel` delas passavam juntas pela
//! conferencia antes de qualquer cobranca. Agora cada chamada RESERVA, ao conferir, a
//! estimativa por cima do que pode gastar (`estimar`: a entrada em bytes de JSON -- nenhum
//! tokenizador de byte gera mais tokens que bytes -- mais o `max_output_tokens`), e a
//! reserva e desfeita depois da cobranca. A chamada so passa se `gasto + reservas em voo`
//! ficar abaixo do teto em cada conta da cadeia; se o que falta e so a reserva das irmas,
//! ela ESPERA a vez em vez de parar (o teto ainda nao bateu). Com folga, as filhas seguem
//! em paralelo; perto do teto, uma de cada vez. Escolhido no lugar de serializar toda
//! conferencia porque serializar mataria o paralelo do `parallel_research` em toda tarefa
//! com orcamento, inclusive longe do teto. O que a garantia NAO cobre, dito: provedor que
//! ignora o `max_output_tokens`, e a chamada aninhada em outra em voo (o classificador
//! dentro da chamada do laco), que reserva sem esperar -- esperar ali seria esperar por si
//! mesma. Chamada feita num `tokio::spawn` dentro da ferramenta sai da tarefa do tokio e
//! do `ESCOPO`: nao e cobrada. Nenhuma ferramenta daqui faz isso; quem fizer, propague.
//!
//! Orcamento em dinheiro sem preco para o modelo nao se confere: a criacao o recusa, e o
//! motor para no primeiro gasto que nao tem preco (`custo` nao medido) em vez de seguir
//! gastando o que ninguem consegue somar.
//!
//! **O «teto global» e por PEDIDO, nao a soma do processo.** `orcamento.teto_*` limita o
//! orcamento que cada tarefa ou fluxo pode pedir; duas tarefas de 900 com teto de 1000
//! gastam 1800, e nenhuma conta soma pedidos diferentes. A completacao do editor
//! (`/v1/ide/completar`) nao roda sob tarefa nenhuma: tem teto proprio
//! (`ide.ia_teto_tokens_hora`) e conta no `/metrics` (`ide.rs`).

use crate::custo::TabelaDePrecos;
use phxclaw_agent_core::tarefa::{Gasto, Orcamento};
use phxclaw_agent_core::{BoxFut, Llm, LlmError, LlmOptions, LlmReply, Message, ToolSpec, Usage};
use std::collections::HashMap;
use std::future::Future;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

pub const CHAVE_TAREFA_TOKENS: &str = "orcamento.tarefa_tokens";
pub const CHAVE_TAREFA_CUSTO: &str = "orcamento.tarefa_custo";
pub const CHAVE_FLUXO_TOKENS: &str = "orcamento.fluxo_tokens";
pub const CHAVE_FLUXO_CUSTO: &str = "orcamento.fluxo_custo";
pub const CHAVE_TETO_TOKENS: &str = "orcamento.teto_tokens";
pub const CHAVE_TETO_CUSTO: &str = "orcamento.teto_custo";

/// O que a configuracao manda: padrao da tarefa, padrao do fluxo e o teto global.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Politica {
    pub tarefa: Orcamento,
    pub fluxo: Orcamento,
    pub teto: Orcamento,
}

/// Tarefa ou fluxo: decide qual padrao vale e como a mensagem chama quem bateu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Alvo {
    Tarefa,
    Fluxo,
}

impl Politica {
    /// Da configuracao do processo. Valor que nao e numero >= 0 e erro da montagem, pela
    /// regra de `config::configuracao`: teto escrito que deixa de valer calado e o pior.
    pub fn da_configuracao() -> Result<Self, String> {
        let tokens = |c: &str| -> Result<Option<u64>, String> {
            match crate::config::valor(c)? {
                None => Ok(None),
                Some(v) => v
                    .as_u64()
                    .map(Some)
                    .ok_or_else(|| format!("{c}: {v} não é um inteiro >= 0")),
            }
        };
        let dinheiro = |c: &str| -> Result<Option<f64>, String> {
            match crate::config::valor(c)? {
                None => Ok(None),
                Some(v) => v
                    .as_f64()
                    .filter(|x| x.is_finite() && *x >= 0.0)
                    .map(Some)
                    .ok_or_else(|| format!("{c}: {v} não é um número >= 0")),
            }
        };
        Ok(Self {
            tarefa: Orcamento {
                tokens: tokens(CHAVE_TAREFA_TOKENS)?,
                custo: dinheiro(CHAVE_TAREFA_CUSTO)?,
            },
            fluxo: Orcamento {
                tokens: tokens(CHAVE_FLUXO_TOKENS)?,
                custo: dinheiro(CHAVE_FLUXO_CUSTO)?,
            },
            teto: Orcamento {
                tokens: tokens(CHAVE_TETO_TOKENS)?,
                custo: dinheiro(CHAVE_TETO_CUSTO)?,
            },
        })
    }

    /// Orcamento em dinheiro configurado sem tabela de precos nao se confere nunca: e erro
    /// da montagem, e nao uma recusa repetida em cada tarefa.
    pub fn conferir_com(&self, tabela: Option<&TabelaDePrecos>) -> Result<(), String> {
        if tabela.is_some() {
            return Ok(());
        }
        for (o, chave) in [
            (&self.tarefa, CHAVE_TAREFA_CUSTO),
            (&self.fluxo, CHAVE_FLUXO_CUSTO),
            (&self.teto, CHAVE_TETO_CUSTO),
        ] {
            if o.custo.is_some() {
                return Err(format!(
                    "{chave} definido sem tabela de preços (custo.precos): orçamento em \
dinheiro não se confere sem preço"
                ));
            }
        }
        Ok(())
    }

    fn padrao(&self, alvo: Alvo) -> &Orcamento {
        match alvo {
            Alvo::Tarefa => &self.tarefa,
            Alvo::Fluxo => &self.fluxo,
        }
    }

    /// O orcamento de um PEDIDO (API): o do pedido, senao o padrao, cada dimensao no teto.
    /// Pedido acima do teto e recusado com o teto escrito; pedido que omite uma dimensao
    /// recebe o padrao, e o teto vale nela do mesmo jeito -- omitir nao e escapar.
    pub fn do_pedido(
        &self,
        pedido: Option<&Orcamento>,
        alvo: Alvo,
    ) -> Result<Option<Orcamento>, String> {
        if let Some(p) = pedido {
            if let (Some(t), Some(teto)) = (p.tokens, self.teto.tokens)
                && t > teto
            {
                return Err(format!(
                    "orçamento de {t} tokens passa do teto global de {teto} tokens \
(orcamento.teto_tokens)"
                ));
            }
            if let Some(c) = p.custo {
                if !(c.is_finite() && c >= 0.0) {
                    return Err(format!("orçamento em dinheiro {c} não é um número >= 0"));
                }
                if let Some(teto) = self.teto.custo
                    && c > teto
                {
                    return Err(format!(
                        "orçamento de {} passa do teto global de {} (orcamento.teto_custo)",
                        crate::custo::valor(c),
                        crate::custo::valor(teto)
                    ));
                }
            }
        }
        Ok(self.efetivo(pedido, alvo))
    }

    /// O mesmo calculo sem recusa, para o motor: o que passou do teto e CORTADO no teto.
    /// So alcanca quem nao passou pela API (a API ja recusou).
    pub fn efetivo(&self, pedido: Option<&Orcamento>, alvo: Alvo) -> Option<Orcamento> {
        let padrao = self.padrao(alvo);
        let p = pedido.cloned().unwrap_or_default();
        let o = Orcamento {
            tokens: menor(p.tokens.or(padrao.tokens), self.teto.tokens),
            custo: menor_f(p.custo.or(padrao.custo), self.teto.custo),
        };
        (!o.vazio()).then_some(o)
    }
}

fn menor(a: Option<u64>, b: Option<u64>) -> Option<u64> {
    match (a, b) {
        (Some(x), Some(y)) => Some(x.min(y)),
        (x, y) => x.or(y),
    }
}

fn menor_f(a: Option<f64>, b: Option<f64>) -> Option<f64> {
    match (a, b) {
        (Some(x), Some(y)) => Some(x.min(y)),
        (x, y) => x.or(y),
    }
}

/// Recusa da criacao: orcamento em dinheiro so se confere se TODO provedor que pode
/// atender (`Llm::provedores`: um, ou a cadeia da rota) tem preco. Basta um sem preco para
/// recusar, dizendo qual: a rota que trocar para ele gastaria o que ninguem soma.
pub fn conferir_preco(
    o: Option<&Orcamento>,
    tabela: Option<&TabelaDePrecos>,
    provedores: &[String],
) -> Result<(), String> {
    if o.and_then(|o| o.custo).is_none() {
        return Ok(());
    }
    let todos = provedores.join(", ");
    let Some(t) = tabela else {
        return Err(format!(
            "orçamento em dinheiro sem tabela de preços (custo.precos): não dá para conferir o \
gasto de {todos}"
        ));
    };
    let sem: Vec<&str> = provedores
        .iter()
        .filter(|p| t.preco(p).is_none())
        .map(String::as_str)
        .collect();
    match sem.as_slice() {
        [] => Ok(()),
        [um] if provedores.len() == 1 => Err(format!(
            "orçamento em dinheiro para {um}, que não tem preço na tabela de preços: não dá \
para conferir o gasto"
        )),
        _ => Err(format!(
            "orçamento em dinheiro para a rota {todos}: {} não tem preço na tabela de preços; \
não dá para conferir o gasto se a rota trocar para ele",
            sem.join(", ")
        )),
    }
}

// ------------------------------------------------------------------ a conta em execucao

struct Conta {
    /// Geracao da abertura: a mesma tarefa reaberta (retomada) nao apaga a conta nova
    /// quando a velha fecha.
    geracao: u64,
    pai: Option<String>,
    alvo: Alvo,
    limite: Option<Orcamento>,
    gasto: Gasto,
    /// O que as chamadas em voo desta conta e das descendentes reservaram e ainda nao
    /// cobraram: e o que a irma ve antes de passar pela conferencia.
    reservado: Previsto,
    /// Chamadas cobradas POR DENTRO (ferramenta, classificador) que o motor ainda nao
    /// registrou na tarefa: a conta ja as somou; falta o `custo` da tarefa e a evidencia.
    por_dentro: Vec<(String, Usage)>,
}

fn contas() -> &'static Mutex<HashMap<String, Conta>> {
    static C: OnceLock<Mutex<HashMap<String, Conta>>> = OnceLock::new();
    C.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Acorda quem espera a vez quando uma reserva e desfeita.
fn avisos() -> &'static tokio::sync::Notify {
    static N: OnceLock<tokio::sync::Notify> = OnceLock::new();
    N.get_or_init(tokio::sync::Notify::new)
}

static GERACAO: AtomicU64 = AtomicU64::new(1);

/// Profundidade maxima da cadeia de contas: mais que isto e laco de `parent`, nao familia.
const CADEIA_MAX: usize = 64;

/// A conta aberta de uma tarefa em execucao; fechar e soltar (`Drop`).
#[derive(Debug)]
pub struct Aberta {
    id: String,
    geracao: u64,
}

/// A parada: o que bateu, dito em texto para o `error` da tarefa.
#[derive(Debug, Clone, PartialEq)]
pub struct Estouro(pub String);

/// O maximo que uma chamada pode gastar, estimado por cima ANTES dela.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Previsto {
    pub tokens: u64,
    pub custo: f64,
}

/// A estimativa por cima de uma chamada: a entrada e o pedido inteiro em bytes de JSON
/// (mensagens, imagens em base64 e ferramentas -- nenhum tokenizador de byte gera mais
/// tokens que bytes, e o JSON carrega mais que o modelo le), a saida e o
/// `max_output_tokens`, e o dinheiro e o do provedor MAIS CARO de `provedores` (antes da
/// chamada nao se sabe qual vai atender). Provedor sem preco nao entra no dinheiro: a
/// cobranca dele deixa a conta «nao medida» e para a tarefa de todo jeito.
pub fn estimar(
    msgs: &[Message],
    specs: &[ToolSpec],
    opcoes: &LlmOptions,
    provedores: &[String],
    precos: Option<&TabelaDePrecos>,
) -> Previsto {
    let bytes = |v: Result<Vec<u8>, serde_json::Error>| v.map_or(0, |b| b.len() as u64);
    let entrada = bytes(serde_json::to_vec(msgs)) + bytes(serde_json::to_vec(specs));
    let saida = u64::from(opcoes.max_output_tokens);
    let (pe, ps) = precos.map_or((0.0, 0.0), |t| {
        provedores
            .iter()
            .filter_map(|p| t.preco(p))
            .fold((0.0f64, 0.0f64), |(e, s), p| {
                (e.max(p.entrada), s.max(p.saida))
            })
    });
    Previsto {
        tokens: entrada + saida,
        custo: (entrada as f64 * pe + saida as f64 * ps) / 1e6,
    }
}

/// A reserva de uma chamada em voo, em cada conta da cadeia; soltar desfaz e acorda quem
/// espera a vez. A cobranca vem ANTES de soltar: no meio, a chamada conta duas vezes (por
/// cima), nunca nenhuma.
#[derive(Debug)]
pub struct Reserva {
    contas: Vec<(String, u64)>,
    previsto: Previsto,
}

impl Drop for Reserva {
    fn drop(&mut self) {
        {
            let mut g = trava();
            for (id, geracao) in &self.contas {
                if let Some(c) = g.get_mut(id).filter(|c| c.geracao == *geracao) {
                    c.reservado.tokens = c.reservado.tokens.saturating_sub(self.previsto.tokens);
                    c.reservado.custo = (c.reservado.custo - self.previsto.custo).max(0.0);
                }
            }
        }
        avisos().notify_waiters();
    }
}

/// Os ids da cadeia, da conta `id` ate a raiz aberta.
fn cadeia(g: &HashMap<String, Conta>, id: &str) -> Vec<String> {
    let mut v = Vec::new();
    let mut atual = Some(id.to_string());
    while let Some(i) = atual.take() {
        if v.len() >= CADEIA_MAX {
            break;
        }
        let Some(c) = g.get(&i) else { break };
        atual = c.pai.clone();
        v.push(i);
    }
    v
}

/// A conta (ou uma ancestral) ja bateu? E se nao bateu: so passa esperando a vez?
fn avaliar(g: &HashMap<String, Conta>, id: &str) -> Result<bool, Estouro> {
    let mut ocupada = false;
    for i in cadeia(g, id) {
        let c = &g[&i];
        if let Some(e) = bateu(&i, c, i == id) {
            return Err(e);
        }
        ocupada |= ocupada_por_irmas(c);
    }
    Ok(ocupada)
}

/// O teto ainda nao bateu, mas as reservas em voo o alcancam: quem chega agora espera.
fn ocupada_por_irmas(c: &Conta) -> bool {
    let Some(lim) = &c.limite else { return false };
    let r = c.reservado;
    let tokens = lim
        .tokens
        .is_some_and(|t| r.tokens > 0 && c.gasto.tokens.saturating_add(r.tokens) >= t);
    let custo = match (lim.custo, c.gasto.custo) {
        (Some(teto), Some(g)) => r.custo > 0.0 && g + r.custo >= teto,
        _ => false,
    };
    tokens || custo
}

/// Conferir e reservar a chamada da conta `id`. Sem teto na cadeia, nada a reservar e a
/// estimativa nem se calcula (`None`): orcamento desligado nao paga serializar o pedido.
async fn reservar_id(
    id: &str,
    previsto: impl FnOnce() -> Previsto,
    pode_esperar: bool,
) -> Result<Option<Reserva>, Estouro> {
    {
        let g = trava();
        if !cadeia(&g, id).iter().any(|i| g[i].limite.is_some()) {
            return Ok(None);
        }
    }
    let previsto = previsto();
    loop {
        // Registrado ANTES de olhar a conta: a reserva desfeita entre a olhada e a espera
        // nao se perde.
        let aviso = avisos().notified();
        tokio::pin!(aviso);
        aviso.as_mut().enable();
        {
            let mut g = trava();
            let ocupada = avaliar(&g, id)?;
            if !ocupada || !pode_esperar {
                let mut contas = Vec::new();
                for i in cadeia(&g, id) {
                    if let Some(c) = g.get_mut(&i) {
                        c.reservado.tokens = c.reservado.tokens.saturating_add(previsto.tokens);
                        c.reservado.custo += previsto.custo;
                        contas.push((i, c.geracao));
                    }
                }
                return Ok(Some(Reserva { contas, previsto }));
            }
        }
        // O prazo e so rede de seguranca contra aviso perdido; o normal e o aviso.
        let _ = tokio::time::timeout(Duration::from_millis(250), aviso).await;
    }
}

/// Cobra uma chamada na conta `id` e em todas as ancestrais abertas, e diz se alguem
/// bateu. `custo` ausente e «nao medido» e contamina a soma para sempre. `por_dentro`
/// fica na conta propria para o motor registrar na tarefa.
fn cobrar_id(
    id: &str,
    tokens: u64,
    custo: Option<f64>,
    moeda: Option<&str>,
    por_dentro: Option<(String, Usage)>,
) -> Result<(), Estouro> {
    let mut g = trava();
    if let (Some(c), Some(p)) = (g.get_mut(id), por_dentro) {
        c.por_dentro.push(p);
    }
    let mut primeiro = None;
    for i in cadeia(&g, id) {
        let Some(c) = g.get_mut(&i) else { break };
        c.gasto.tokens = c.gasto.tokens.saturating_add(tokens);
        c.gasto.custo = match (c.gasto.custo, custo) {
            (Some(a), Some(b)) => Some(a + b),
            _ => None,
        };
        if c.gasto.moeda.is_none() {
            c.gasto.moeda = moeda.map(str::to_string);
        }
        if primeiro.is_none() {
            primeiro = bateu(&i, c, i == id);
        }
    }
    primeiro.map_or(Ok(()), Err)
}

impl Aberta {
    pub fn id(&self) -> &str {
        &self.id
    }

    /// O gasto atual (proprio + descendentes), para gravar na tarefa.
    pub fn gasto(&self) -> Gasto {
        trava()
            .get(&self.id)
            .map(|c| c.gasto.clone())
            .unwrap_or_default()
    }

    /// Antes de gastar: a conta ou um ancestral ja bateu (uma filha gastou por ela)?
    pub fn conferir(&self) -> Result<(), Estouro> {
        avaliar(&trava(), &self.id).map(|_| ())
    }

    /// `conferir` para uma chamada que vai sair agora: reserva a estimativa dela (so
    /// calculada se houver teto na cadeia) e espera a vez se as irmas em voo ja alcancam o
    /// teto. Solte a reserva DEPOIS de `cobrar`.
    pub async fn reservar(
        &self,
        previsto: impl FnOnce() -> Previsto,
    ) -> Result<Option<Reserva>, Estouro> {
        reservar_id(&self.id, previsto, true).await
    }

    /// Cobra uma chamada desta tarefa nela e em todos os ancestrais abertos, e diz se
    /// alguem bateu. `custo` ausente e «nao medido» e contamina a soma para sempre.
    pub fn cobrar(
        &self,
        tokens: u64,
        custo: Option<f64>,
        moeda: Option<&str>,
    ) -> Result<(), Estouro> {
        cobrar_id(&self.id, tokens, custo, moeda, None)
    }

    /// As chamadas cobradas por dentro desde a ultima vez (modelo que atendeu e uso), para
    /// o motor registrar no custo e na evidencia da tarefa.
    pub fn drenar_por_dentro(&self) -> Vec<(String, Usage)> {
        trava()
            .get_mut(&self.id)
            .map(|c| std::mem::take(&mut c.por_dentro))
            .unwrap_or_default()
    }
}

impl Drop for Aberta {
    fn drop(&mut self) {
        let mut g = trava();
        if g.get(&self.id).is_some_and(|c| c.geracao == self.geracao) {
            g.remove(&self.id);
        }
    }
}

fn trava() -> std::sync::MutexGuard<'static, HashMap<String, Conta>> {
    contas().lock().unwrap_or_else(|p| p.into_inner())
}

/// Abre a conta de uma tarefa. `inicial` e o gasto ja gravado (a retomada continua de
/// onde parou, nao de zero). A conta comeca com custo 0 medido; quem comecou sem medir
/// (`inicial.custo` ausente com tokens gastos) continua nao medido.
pub fn abrir(
    id: &str,
    pai: Option<&str>,
    alvo: Alvo,
    limite: Option<Orcamento>,
    inicial: Gasto,
) -> Aberta {
    let geracao = GERACAO.fetch_add(1, Ordering::Relaxed);
    let gasto = Gasto {
        custo: if inicial.tokens == 0 {
            Some(inicial.custo.unwrap_or(0.0))
        } else {
            inicial.custo
        },
        ..inicial
    };
    trava().insert(
        id.to_string(),
        Conta {
            geracao,
            pai: pai.map(str::to_string),
            alvo,
            limite,
            gasto,
            reservado: Previsto::default(),
            por_dentro: Vec::new(),
        },
    );
    Aberta {
        id: id.to_string(),
        geracao,
    }
}

// ------------------------------------------------------------------ a chamada por dentro

/// Sob qual conta corre o codigo desta tarefa do tokio. `dentro`: ja ha uma chamada ao
/// modelo em voo acima (a do laco, ou uma cobrada por dentro), que reservou e cobra a
/// propria resposta.
#[derive(Clone)]
struct Escopo {
    conta: String,
    precos: Option<Arc<TabelaDePrecos>>,
    dentro: bool,
}

tokio::task_local! {
    static ESCOPO: Escopo;
}

/// Roda `f` (uma ferramenta) sob a conta da tarefa `id`, se ela estiver aberta: o modelo
/// que a ferramenta chamar por dentro cobra esta conta. Sem conta aberta (o `mcp-serve`,
/// que nao executa tarefa), roda como antes.
pub async fn sob_a_conta<F: Future>(
    id: &str,
    precos: Option<Arc<TabelaDePrecos>>,
    f: F,
) -> F::Output {
    if !trava().contains_key(id) {
        return f.await;
    }
    let e = Escopo {
        conta: id.to_string(),
        precos,
        dentro: false,
    };
    ESCOPO.scope(e, f).await
}

/// Roda `f` (a chamada do proprio laco) sob a conta: o motor cobra a resposta com o preco
/// de quem atendeu, e o envolto a deixa passar; o classificador da rota, la dentro,
/// continua cobrando.
pub async fn na_chamada_do_motor<F: Future>(
    conta: &Aberta,
    precos: Option<Arc<TabelaDePrecos>>,
    f: F,
) -> F::Output {
    let e = Escopo {
        conta: conta.id.clone(),
        precos,
        dentro: true,
    };
    ESCOPO.scope(e, f).await
}

/// O modelo que cobra a conta da tarefa sob a qual e chamado.
pub struct LlmDaTarefa {
    interno: Arc<dyn Llm>,
    /// `false` (a raiz, da montagem): a chamada do laco passa sem cobrar, porque o motor
    /// a cobra. `true` (o classificador): cobra sempre -- o uso dele nunca esta na
    /// resposta que o motor ve.
    sempre: bool,
}

impl LlmDaTarefa {
    /// O modelo da montagem: UM involucro, e o mesmo `Arc` vai ao agente, as ferramentas e
    /// aos subagentes. Envolver duas vezes cobraria duas vezes a mesma chamada.
    pub fn envolver(llm: Arc<dyn Llm>) -> Arc<dyn Llm> {
        Arc::new(Self {
            interno: llm,
            sempre: false,
        })
    }

    /// Um modelo auxiliar (o classificador da rota) que gasta dentro da chamada de outro.
    pub fn por_dentro(llm: Arc<dyn Llm>) -> Arc<dyn Llm> {
        Arc::new(Self {
            interno: llm,
            sempre: true,
        })
    }
}

impl Llm for LlmDaTarefa {
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
            match ESCOPO.try_with(Escopo::clone).ok() {
                Some(e) if self.sempre || !e.dentro => {
                    cobrada(self.interno.as_ref(), e, messages, tools, options).await
                }
                _ => self.interno.chat(messages, tools, options).await,
            }
        })
    }
}

/// A chamada cobrada: reserva (conferindo), chama, cobra com o preco de quem atendeu,
/// solta. Estouro vira `LlmError::Denied` -- a ferramenta falha, e o motor, que confere a
/// conta depois de cada ferramenta, para a tarefa em `budget_exceeded`.
async fn cobrada(
    llm: &dyn Llm,
    e: Escopo,
    messages: &[Message],
    tools: &[ToolSpec],
    options: &LlmOptions,
) -> Result<LlmReply, LlmError> {
    let precos = e.precos.clone();
    let reserva = reservar_id(
        &e.conta,
        || {
            estimar(
                messages,
                tools,
                options,
                &llm.provedores(),
                precos.as_deref(),
            )
        },
        !e.dentro,
    )
    .await
    .map_err(|x| LlmError::Denied(x.0))?;
    let conta = e.conta.clone();
    let dentro = Escopo { dentro: true, ..e };
    let (r, diario) = ESCOPO
        .scope(
            dentro,
            crate::roteamento::com_diario(llm.chat(messages, tools, options)),
        )
        .await;
    let modelo = diario
        .last()
        .and_then(|a| a.atendeu.clone())
        .unwrap_or_else(|| llm.id());
    crate::roteamento::reanotar(diario);
    let reply = r?;
    let uso = &reply.usage;
    let tabela = precos.as_deref();
    let cobrou = cobrar_id(
        &conta,
        uso.input_tokens + uso.output_tokens,
        tabela.and_then(|t| t.chamada(&modelo, uso).custo),
        tabela.map(|t| t.moeda.as_str()),
        Some((modelo, uso.clone())),
    );
    drop(reserva);
    cobrou.map_err(|x| LlmError::Denied(x.0))?;
    Ok(reply)
}

/// Quem bateu, em texto: quanto gastou, qual teto, e de quem (esta tarefa, ou o fluxo /
/// a tarefa-mae acima dela).
fn bateu(id: &str, c: &Conta, propria: bool) -> Option<Estouro> {
    let lim = c.limite.as_ref()?;
    let dono = match (propria, c.alvo) {
        (true, Alvo::Tarefa) => "da tarefa".to_string(),
        (true, Alvo::Fluxo) => "do fluxo".to_string(),
        (false, Alvo::Tarefa) => format!("da tarefa-mãe {id}"),
        (false, Alvo::Fluxo) => format!("do fluxo {id}"),
    };
    let moeda = c.gasto.moeda.as_deref().unwrap_or("(moeda da tabela)");
    if let Some(t) = lim.tokens
        && c.gasto.tokens >= t
    {
        return Some(Estouro(format!(
            "orçamento {dono} esgotado: gastou {} tokens, teto de {t} tokens",
            c.gasto.tokens
        )));
    }
    if let Some(teto) = lim.custo {
        match c.gasto.custo {
            Some(g) if g >= teto => {
                return Some(Estouro(format!(
                    "orçamento {dono} esgotado: gastou {} {moeda}, teto de {} {moeda} ({} tokens)",
                    crate::custo::valor(g),
                    crate::custo::valor(teto),
                    c.gasto.tokens
                )));
            }
            None => {
                return Some(Estouro(format!(
                    "orçamento em dinheiro {dono} não conferível: uma chamada saiu sem preço na \
tabela; parou depois de {} tokens",
                    c.gasto.tokens
                )));
            }
            _ => {}
        }
    }
    None
}

#[cfg(test)]
mod testes {
    use super::*;

    fn o(t: Option<u64>, c: Option<f64>) -> Orcamento {
        Orcamento {
            tokens: t,
            custo: c,
        }
    }

    fn id() -> String {
        uuid::Uuid::now_v7().to_string()
    }

    /// Guarda R3: o pedido nao ultrapassa o teto global -- acima dele a criacao recusa
    /// dizendo o teto; omitir a dimensao nao escapa (recebe o teto); e o motor, que nao
    /// recusa, corta no teto.
    #[test]
    fn pedido_nao_ultrapassa_o_teto_global() {
        let p = Politica {
            tarefa: o(Some(500), None),
            fluxo: o(None, None),
            teto: o(Some(1_000), Some(2.0)),
        };
        let e = p
            .do_pedido(Some(&o(Some(5_000), None)), Alvo::Tarefa)
            .unwrap_err();
        assert!(e.contains("teto global de 1000 tokens"), "{e}");
        let e = p
            .do_pedido(Some(&o(None, Some(3.0))), Alvo::Tarefa)
            .unwrap_err();
        assert!(e.contains("teto global"), "{e}");
        // Dentro do teto, vale o pedido; a dimensao omitida recebe o teto.
        assert_eq!(
            p.do_pedido(Some(&o(Some(800), None)), Alvo::Tarefa)
                .unwrap(),
            Some(o(Some(800), Some(2.0)))
        );
        // Sem pedido, o padrao da tarefa (no teto).
        assert_eq!(
            p.do_pedido(None, Alvo::Tarefa).unwrap(),
            Some(o(Some(500), Some(2.0)))
        );
        // O motor corta no teto o que nao passou pela API.
        assert_eq!(
            p.efetivo(Some(&o(Some(9_999), Some(9.0))), Alvo::Fluxo),
            Some(o(Some(1_000), Some(2.0)))
        );
        // Sem nada configurado e sem pedido: sem orcamento.
        assert_eq!(Politica::default().efetivo(None, Alvo::Tarefa), None);
    }

    #[test]
    fn dinheiro_sem_preco_e_recusado_dizendo_por_que() {
        let tab = crate::custo::TabelaDePrecos::de_texto(
            r#"{"moeda":"USD","modelos":{"a:b":{"entrada":1,"saida":1,"data":"2026-10-01","fonte":"f"}}}"#,
        )
        .unwrap();
        let din = o(None, Some(1.0));
        let v = |xs: &[&str]| xs.iter().map(|x| x.to_string()).collect::<Vec<_>>();
        assert!(conferir_preco(Some(&din), Some(&tab), &v(&["a:b"])).is_ok());
        let e = conferir_preco(Some(&din), Some(&tab), &v(&["x:y"])).unwrap_err();
        assert!(e.contains("x:y") && e.contains("não tem preço"), "{e}");
        let e = conferir_preco(Some(&din), None, &v(&["a:b"])).unwrap_err();
        assert!(e.contains("sem tabela"), "{e}");
        // A rota: basta um provedor da cadeia sem preco, e a recusa diz qual.
        let e = conferir_preco(Some(&din), Some(&tab), &v(&["a:b", "x:y"])).unwrap_err();
        assert!(e.contains("rota") && e.contains("x:y não tem preço"), "{e}");
        // Orcamento so de tokens nao precisa de preco.
        assert!(conferir_preco(Some(&o(Some(10), None)), None, &v(&["x:y"])).is_ok());
        let p = Politica {
            tarefa: din,
            ..Politica::default()
        };
        assert!(
            p.conferir_com(None)
                .unwrap_err()
                .contains("orcamento.tarefa_custo")
        );
    }

    /// A filha cobra a mae: o orcamento da mae nao se contorna abrindo subagentes.
    #[test]
    fn a_filha_cobra_a_conta_da_mae_e_para_quando_a_mae_bate() {
        let (m, f) = (id(), id());
        let mae = abrir(
            &m,
            None,
            Alvo::Fluxo,
            Some(o(Some(100), None)),
            Gasto::default(),
        );
        let filha = abrir(&f, Some(&m), Alvo::Tarefa, None, Gasto::default());
        assert!(filha.cobrar(60, None, None).is_ok());
        let e = filha.cobrar(60, None, None).unwrap_err();
        assert!(
            e.0.contains(&format!("do fluxo {m}")) && e.0.contains("120 tokens"),
            "{}",
            e.0
        );
        assert_eq!(mae.gasto().tokens, 120);
        assert!(mae.conferir().unwrap_err().0.contains("do fluxo esgotado"));
        drop(filha);
        // Fechada a filha, a conta dela some e a da mae fica.
        assert!(trava().get(&f).is_none() && trava().get(&m).is_some());
    }

    #[test]
    fn custo_nao_medido_com_teto_em_dinheiro_para() {
        let a = abrir(
            &id(),
            None,
            Alvo::Tarefa,
            Some(o(None, Some(1.0))),
            Gasto::default(),
        );
        assert!(a.cobrar(10, Some(0.1), Some("USD")).is_ok());
        let e = a.cobrar(10, None, Some("USD")).unwrap_err();
        assert!(
            e.0.contains("não conferível") && e.0.contains("20 tokens"),
            "{}",
            e.0
        );
    }
}
