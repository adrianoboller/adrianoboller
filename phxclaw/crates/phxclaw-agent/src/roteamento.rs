//! Roteamento e troca de provedor por politica (radar R4).
//!
//! Um `Llm` que envolve varios: a cada chamada resolve a cadeia de provedores pela regra
//! deterministica da politica (tipo da tarefa, custo declarado, ordem), tenta o primeiro e
//! troca para o seguinte so quando a falha e do PROVEDOR -- prazo, 429, 5xx, transporte --,
//! ate o teto de trocas. O que decide e escrito, nao inferido:
//!
//! - **Entra pedida, nao imposta.** So o modelo `rota` ou `rota:<spec>` passa por aqui; o
//!   resto continua indo direto ao provedor. Uma politica no arquivo nao muda o modelo de
//!   ninguem que nao a pediu -- e a medicao (`phxclaw avaliar`) continua medindo um modelo
//!   so, sem troca escondida no numero.
//! - **Erro do pedido nao troca.** 4xx (fora 408 e 429), politica negada e credencial
//!   faltando chegam a quem chamou como chegariam sem rota: trocar esconderia o defeito do
//!   argumento -- ou, no caso da politica, a contornaria por outro provedor.
//! - **Toda chamada diz quem atendeu e por que trocou.** O diario sai por `com_diario`, que
//!   o motor usa em volta do `chat`, e vira passo `modelo` da tarefa e linha da evidencia.
//!   Sem o motor em volta (uso como biblioteca), o diario nao tem para onde ir e nada se
//!   perde alem dele: a chamada funciona igual.
//! - **O tipo da tarefa sai do decisor (R1), e o decisor nunca escolhe provedor.** A escada
//!   so classifica o objetivo numa das chaves de `por_tipo`; sem decisao confiante vale o
//!   `padrao`, e a lista de provedores continua sendo a do arquivo. Os degraus, do mais
//!   barato ao mais caro: `regras`, o Laya (`"laya": true`, um forward pass do
//!   `laya-serve`, configurado em `decisao.laya.*`) e o `modelo` em modo restrito.

use crate::decisao::{
    DecisorDeRegras, DecisorPorModelo, Degrau, Encaminhamento, Escada, Questao, RegraDeDecisao,
    Valor,
};
use crate::tarefa::{StepRecord, Task};
use chrono::Utc;
use phxclaw_agent_core::{
    BoxFut, Llm, LlmError, LlmOptions, LlmReply, Message, Role, ToolSpec, truncate_for_model,
};
use phxclaw_evidence_ledger::{EvidenceDraft, EvidenceLedger, EvidenceOutcome};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// A chave do `config.json` com o caminho do arquivo da politica.
pub const CHAVE: &str = "modelo.roteamento";
/// O modelo que pede a rota: `rota` (a politica decide tudo) ou `rota:<spec>` (o `spec`
/// vem primeiro e a politica da a reserva).
pub const SPEC_ROTA: &str = "rota";
/// Teto do teto: mais trocas que isto numa chamada e laco, nao reserva.
const TROCAS_MAX_TETO: u32 = 8;

/// Por que um provedor falhou, nas classes que a politica pode mandar trocar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub enum Falha {
    #[serde(rename = "timeout")]
    Timeout,
    #[serde(rename = "429")]
    Limite,
    #[serde(rename = "5xx")]
    Servidor,
    #[serde(rename = "transporte")]
    Transporte,
    #[serde(rename = "resposta_invalida")]
    RespostaInvalida,
}

impl Falha {
    pub fn nome(self) -> &'static str {
        match self {
            Falha::Timeout => "timeout",
            Falha::Limite => "429",
            Falha::Servidor => "5xx",
            Falha::Transporte => "transporte",
            Falha::RespostaInvalida => "resposta_invalida",
        }
    }
    fn de_nome(n: &str) -> Option<Self> {
        [
            Falha::Timeout,
            Falha::Limite,
            Falha::Servidor,
            Falha::Transporte,
            Falha::RespostaInvalida,
        ]
        .into_iter()
        .find(|f| f.nome() == n)
    }
}

/// O erro do provedor visto pela politica.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Classe {
    Trocavel(Falha),
    /// Nunca troca, qualquer que seja a politica; o texto e o motivo registrado.
    NaoTroca(&'static str),
}

pub fn classificar(e: &LlmError) -> Classe {
    match e {
        LlmError::Api { status: 429, .. } => Classe::Trocavel(Falha::Limite),
        LlmError::Api { status: 408, .. } => Classe::Trocavel(Falha::Timeout),
        LlmError::Api { status, .. } if (500..600).contains(status) => {
            Classe::Trocavel(Falha::Servidor)
        }
        LlmError::Api { .. } => Classe::NaoTroca(
            "erro do pedido (4xx): outro provedor esconderia o defeito do argumento",
        ),
        // O transporte unico dos provedores marca o prazo estourado com «(timeout)».
        LlmError::Transport(t) if t.contains("(timeout)") => Classe::Trocavel(Falha::Timeout),
        LlmError::Transport(_) => Classe::Trocavel(Falha::Transporte),
        LlmError::Parse(_) => Classe::Trocavel(Falha::RespostaInvalida),
        LlmError::Denied(_) => {
            Classe::NaoTroca("negado por politica: outro provedor seria contornar a politica")
        }
        LlmError::Credential(_) => {
            Classe::NaoTroca("credencial: o operador tem de ver, nao a reserva esconder")
        }
    }
}

// ------------------------------------------------------------------ a politica

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProvedorBruto {
    spec: String,
    #[serde(default)]
    custo: f64,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct ClassificarBruto {
    #[serde(default)]
    regras: Vec<Value>,
    #[serde(default)]
    modelo: Option<String>,
    #[serde(default)]
    limiar: Option<f64>,
    #[serde(default)]
    padrao: Option<String>,
    #[serde(default)]
    laya: bool,
}

// Campo que o leitor nao conhece e recusa, nao silencio: configuracao que nao e lida mente.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct PoliticaBruta {
    provedores: Vec<ProvedorBruto>,
    #[serde(default)]
    por_tipo: BTreeMap<String, Vec<String>>,
    #[serde(default)]
    classificar: Option<ClassificarBruto>,
    #[serde(default)]
    preferir_barato: bool,
    #[serde(default)]
    trocar_em: Option<Vec<String>>,
    #[serde(default)]
    trocas_max: Option<u32>,
    #[serde(default)]
    prazo_s: Option<u64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Provedor {
    pub spec: String,
    /// Custo RELATIVO declarado pelo operador (so ordena; dinheiro e outra conta).
    pub custo: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Classificacao {
    pub regras: Vec<RegraDeDecisao>,
    pub modelo: Option<String>,
    pub limiar: f64,
    pub padrao: Option<String>,
    /// O degrau do Laya entre as regras e o modelo. O limiar dele e o `decisao.laya.limiar`,
    /// nao o `limiar` daqui: a confianca do Laya e probabilidade calibrada, a do modelo e a
    /// que ele declara de si -- o mesmo numero nao quer dizer a mesma coisa nas duas.
    pub laya: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Politica {
    pub provedores: Vec<Provedor>,
    pub por_tipo: BTreeMap<String, Vec<String>>,
    pub classificar: Option<Classificacao>,
    pub preferir_barato: bool,
    pub trocar_em: BTreeSet<Falha>,
    pub trocas_max: u32,
    pub prazo: Option<Duration>,
}

impl Politica {
    /// Le e confere. Todo spec citado em `por_tipo` tem de estar em `provedores` (a lista
    /// de provedores mora num lugar so), e o valor de cada regra de classificacao tem de ser
    /// uma chave de `por_tipo` -- regra que aponta para tipo inexistente nunca casaria nada.
    pub fn de_json(texto: &str) -> Result<Self, String> {
        let b: PoliticaBruta = serde_json::from_str(texto).map_err(|e| e.to_string())?;
        if b.provedores.is_empty() {
            return Err("'provedores' vazia".into());
        }
        let mut vistos = BTreeSet::new();
        for p in &b.provedores {
            if !p.spec.contains(':') || p.spec.starts_with(SPEC_ROTA) {
                return Err(format!("spec invalido em 'provedores': {:?}", p.spec));
            }
            if !vistos.insert(p.spec.clone()) {
                return Err(format!("spec repetido em 'provedores': {}", p.spec));
            }
            if !p.custo.is_finite() || p.custo < 0.0 {
                return Err(format!("custo invalido de {}: {}", p.spec, p.custo));
            }
        }
        for (tipo, lista) in &b.por_tipo {
            if lista.is_empty() {
                return Err(format!("'por_tipo.{tipo}' vazia"));
            }
            if let Some(s) = lista.iter().find(|s| !vistos.contains(*s)) {
                return Err(format!(
                    "'por_tipo.{tipo}' cita {s}, que nao esta em 'provedores'"
                ));
            }
        }
        let trocar_em = match b.trocar_em {
            None => [
                Falha::Timeout,
                Falha::Limite,
                Falha::Servidor,
                Falha::Transporte,
            ]
            .into(),
            Some(v) => v
                .iter()
                .map(|n| {
                    Falha::de_nome(n).ok_or_else(|| {
                        format!(
                            "'trocar_em' desconhecido: {n:?} (timeout, 429, 5xx, transporte, resposta_invalida)"
                        )
                    })
                })
                .collect::<Result<_, _>>()?,
        };
        let trocas_max = b.trocas_max.unwrap_or(2);
        if trocas_max > TROCAS_MAX_TETO {
            return Err(format!(
                "'trocas_max' {trocas_max} acima do teto {TROCAS_MAX_TETO}"
            ));
        }
        let classificar = match b.classificar {
            None => None,
            Some(c) => {
                if b.por_tipo.is_empty() {
                    return Err("'classificar' sem 'por_tipo': nao ha tipo para escolher".into());
                }
                let regras = c
                    .regras
                    .iter()
                    .map(RegraDeDecisao::de_json)
                    .collect::<Result<Vec<_>, _>>()?;
                for r in &regras {
                    match &r.valor {
                        Valor::Escolha(t) if b.por_tipo.contains_key(t) => {}
                        v => {
                            return Err(format!(
                                "regra de classificacao com valor {v:?}: tem de ser uma chave de 'por_tipo'"
                            ));
                        }
                    }
                }
                if let Some(p) = &c.padrao
                    && !b.por_tipo.contains_key(p)
                {
                    return Err(format!("'classificar.padrao' {p:?} nao esta em 'por_tipo'"));
                }
                if let Some(m) = &c.modelo
                    && (!m.contains(':') || m.starts_with(SPEC_ROTA))
                {
                    return Err(format!("'classificar.modelo' invalido: {m:?}"));
                }
                let limiar = c.limiar.unwrap_or(0.7);
                if !(limiar.is_finite() && (0.0..=1.0).contains(&limiar)) {
                    return Err(format!("'classificar.limiar' fora de 0..1: {limiar}"));
                }
                Some(Classificacao {
                    regras,
                    modelo: c.modelo,
                    limiar,
                    padrao: c.padrao,
                    laya: c.laya,
                })
            }
        };
        Ok(Self {
            provedores: b
                .provedores
                .into_iter()
                .map(|p| Provedor {
                    spec: p.spec,
                    custo: p.custo,
                })
                .collect(),
            por_tipo: b.por_tipo,
            classificar,
            preferir_barato: b.preferir_barato,
            trocar_em,
            trocas_max,
            prazo: b.prazo_s.filter(|s| *s > 0).map(Duration::from_secs),
        })
    }

    fn custo(&self, spec: &str) -> f64 {
        self.provedores
            .iter()
            .find(|p| p.spec == spec)
            .map_or(0.0, |p| p.custo)
    }

    /// A ordem dos provedores desta chamada: a cabeca pedida, depois a lista do tipo (ou a
    /// ordem de `provedores`), pelo custo quando o operador prefere o barato. Sem repetir.
    pub fn cadeia(&self, cabeca: Option<&str>, tipo: Option<&str>) -> Vec<String> {
        let mut base: Vec<String> = tipo
            .and_then(|t| self.por_tipo.get(t))
            .cloned()
            .unwrap_or_else(|| self.provedores.iter().map(|p| p.spec.clone()).collect());
        if self.preferir_barato {
            // Estavel: empate de custo fica na ordem que o operador escreveu.
            base.sort_by(|a, b| self.custo(a).total_cmp(&self.custo(b)));
        }
        let mut v: Vec<String> = cabeca.map(str::to_string).into_iter().collect();
        for s in base {
            if !v.contains(&s) {
                v.push(s);
            }
        }
        v
    }
}

/// `rota` -> `Some(None)`; `rota:<spec>` -> `Some(Some(spec))`; outro -> `None`.
pub fn pedido(spec: &str) -> Option<Option<&str>> {
    if spec == SPEC_ROTA {
        return Some(None);
    }
    spec.strip_prefix("rota:").map(Some)
}

// ------------------------------------------------------------------ o diario

/// Uma tentativa dentro da chamada.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Tentativa {
    pub spec: String,
    /// `ok`, o nome da falha (`429`, `5xx`...) ou `nao_troca`.
    pub desfecho: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub erro: Option<String>,
}

/// Uma chamada roteada: o tipo e quem o decidiu, a cadeia, as tentativas, quem atendeu e,
/// quando ninguem atendeu, por que parou de trocar.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Atendimento {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tipo: Option<String>,
    pub tipo_por: String,
    pub cadeia: Vec<String>,
    pub tentativas: Vec<Tentativa>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub atendeu: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parada: Option<String>,
}

impl Atendimento {
    pub fn trocas(&self) -> usize {
        self.tentativas.len().saturating_sub(1)
    }

    pub fn resumo(&self) -> String {
        let mut s = match &self.atendeu {
            Some(a) => format!("atendeu {a}"),
            None => "nenhum provedor atendeu".to_string(),
        };
        if let Some(t) = &self.tipo {
            s.push_str(&format!(" (tipo {t}, por {})", self.tipo_por));
        }
        let falhas: Vec<String> = self
            .tentativas
            .iter()
            .filter(|t| t.desfecho != "ok")
            .map(|t| format!("{} -> {}", t.spec, t.desfecho))
            .collect();
        if !falhas.is_empty() {
            s.push_str(&format!("; trocou: {}", falhas.join(", ")));
        }
        if let Some(p) = &self.parada {
            s.push_str(&format!("; parou: {p}"));
        }
        s
    }
}

tokio::task_local! {
    static DIARIO: RefCell<Vec<Atendimento>>;
}

/// Roda `f` (o `chat` do motor) com um diario proprio: o que os `LlmRoteado` chamados
/// dentro dele anotarem volta junto do resultado. Por tarefa do tokio, e nao global, para
/// duas tarefas em paralelo nao trocarem diario.
pub async fn com_diario<F: std::future::Future>(f: F) -> (F::Output, Vec<Atendimento>) {
    DIARIO
        .scope(RefCell::new(Vec::new()), async move {
            let r = f.await;
            (r, DIARIO.with(|d| d.take()))
        })
        .await
}

fn anotar(a: Atendimento) {
    let _ = DIARIO.try_with(|d| d.borrow_mut().push(a));
}

/// Devolve ao diario de fora o que um `com_diario` de dentro capturou: a cobranca por
/// dentro (`orcamento::LlmDaTarefa`) abre o seu para saber quem atendeu, e o passo
/// `modelo` do motor nao pode sumir por isso.
pub(crate) fn reanotar(diario: Vec<Atendimento>) {
    for a in diario {
        anotar(a);
    }
}

/// O diario de uma volta do laco vira passo `modelo` da tarefa e linha da evidencia.
pub fn registrar(task: &mut Task, passo: u32, ledger: &EvidenceLedger, diario: &[Atendimento]) {
    for a in diario {
        let outcome = match (&a.atendeu, a.trocas()) {
            (Some(_), 0) => "ok",
            (Some(_), _) => "trocou",
            (None, _) => "falhou",
        };
        let corpo = serde_json::to_value(a).unwrap_or(Value::Null);
        task.steps.push(StepRecord {
            n: passo + 1,
            at: Utc::now(),
            kind: "modelo".into(),
            tool: None,
            arguments: corpo.clone(),
            outcome: outcome.into(),
            summary: truncate_for_model(&a.resumo(), 400),
        });
        let _ = ledger.append(EvidenceDraft {
            action_uuid: phxclaw_types::new_uuid_v7(),
            correlation_uuid: task.id.parse().ok(),
            actor: "phxclaw-agent".into(),
            capability: "modelo.roteamento".into(),
            action: "chat".into(),
            outcome: if a.atendeu.is_some() {
                EvidenceOutcome::Succeeded
            } else {
                EvidenceOutcome::Failed
            },
            request_summary: json!({"cadeia": a.cadeia, "tipo": a.tipo, "tipo_por": a.tipo_por}),
            result_summary: corpo,
            artifact_uris: vec![],
        });
    }
}

// ------------------------------------------------------------------ o modelo roteado

/// O tipo lembrado por objetivo: com classificador de modelo, decidir a cada volta do laco
/// pagaria uma chamada a mais por passo para a mesma resposta.
struct TipoLembrado {
    chave: String,
    tipo: Option<String>,
    por: String,
}

const TIPOS_LEMBRADOS: usize = 64;

pub struct LlmRoteado {
    cabeca: Option<String>,
    modelos: BTreeMap<String, Arc<dyn Llm>>,
    politica: Arc<Politica>,
    classificador: Option<Escada>,
    lembrados: Mutex<Vec<TipoLembrado>>,
}

impl LlmRoteado {
    /// Monta todos os provedores da politica (e a cabeca, e o classificador) pela
    /// `fabrica`. Provedor que nao monta (sem chave, spec invalido) e ERRO aqui, nunca
    /// reserva que some calada no dia em que for precisa.
    pub fn montar(
        politica: Politica,
        cabeca: Option<&str>,
        fabrica: impl Fn(&str) -> Result<Arc<dyn Llm>, String>,
    ) -> Result<Self, String> {
        Self::montar_com(politica, cabeca, fabrica, || {
            Err(format!(
                "'classificar.laya' pede o decisor Laya: defina {}",
                crate::decisao::laya::CHAVE_URL
            ))
        })
    }

    /// `montar`, com a fabrica do degrau do Laya (chamada so quando a politica o pede). A
    /// falta dele e erro da montagem, como a do provedor: degrau que some calado no dia em
    /// que e preciso deixa o tipo inteiro cair no `padrao` sem ninguem saber por que.
    pub fn montar_com(
        politica: Politica,
        cabeca: Option<&str>,
        fabrica: impl Fn(&str) -> Result<Arc<dyn Llm>, String>,
        laya: impl FnOnce() -> Result<Degrau, String>,
    ) -> Result<Self, String> {
        if let Some(c) = cabeca
            && (c.starts_with(SPEC_ROTA) || !c.contains(':'))
        {
            return Err(format!("rota: spec da cabeca invalido: {c:?}"));
        }
        let mut modelos = BTreeMap::new();
        let specs = politica
            .provedores
            .iter()
            .map(|p| p.spec.as_str())
            .chain(cabeca);
        for s in specs {
            if !modelos.contains_key(s) {
                modelos.insert(
                    s.to_string(),
                    fabrica(s).map_err(|e| format!("rota: {s}: {e}"))?,
                );
            }
        }
        let classificador = match &politica.classificar {
            None => None,
            Some(c) => {
                let mut degraus = Vec::new();
                if !c.regras.is_empty() {
                    degraus.push(Degrau {
                        decisor: Arc::new(DecisorDeRegras {
                            nome: "roteamento".into(),
                            regras: c.regras.clone(),
                        }),
                        limiar: c.limiar,
                    });
                }
                if c.laya {
                    degraus.push(laya().map_err(|e| format!("rota: classificador laya: {e}"))?);
                }
                if let Some(m) = &c.modelo {
                    let llm = fabrica(m).map_err(|e| format!("rota: classificador {m}: {e}"))?;
                    // O classificador gasta DENTRO da chamada do laco, e o uso dele nao
                    // esta na resposta que o motor cobra: cobra a conta da tarefa por si.
                    let llm = crate::orcamento::LlmDaTarefa::por_dentro(llm);
                    degraus.push(Degrau {
                        decisor: Arc::new(DecisorPorModelo { llm }),
                        limiar: c.limiar,
                    });
                }
                // Sem pessoa: classificar a tarefa nao vale parar quem a pediu; o incerto
                // vai para o `padrao`.
                Some(Escada {
                    degraus,
                    pessoa: false,
                })
            }
        };
        Ok(Self {
            cabeca: cabeca.map(str::to_string),
            modelos,
            politica: Arc::new(politica),
            classificador,
            lembrados: Mutex::new(Vec::new()),
        })
    }

    async fn tipo_de(&self, messages: &[Message]) -> (Option<String>, String) {
        let Some(escada) = &self.classificador else {
            return (None, "sem classificacao".into());
        };
        let objetivo = messages
            .iter()
            .find(|m| m.role == Role::User)
            .map(|m| m.content.as_str())
            .unwrap_or("");
        let chave = crate::motor::sha256_hex(objetivo.as_bytes());
        {
            let g = self.lembrados.lock().unwrap_or_else(|p| p.into_inner());
            if let Some(t) = g.iter().find(|t| t.chave == chave) {
                return (t.tipo.clone(), t.por.clone());
            }
        }
        let tipos: Vec<&str> = self.politica.por_tipo.keys().map(String::as_str).collect();
        let q = Questao::escolha("task type", &tipos, objetivo);
        let padrao = self
            .politica
            .classificar
            .as_ref()
            .and_then(|c| c.padrao.clone());
        let (tipo, por) = match escada.decidir(&q).await.fim {
            Encaminhamento::Decidido(d) => match d.valor {
                Valor::Escolha(t) => (Some(t), format!("{} ({:.2})", d.decisor, d.confianca)),
                _ => (padrao, "padrao".to_string()),
            },
            _ => (padrao, "padrao (sem decisao confiante)".to_string()),
        };
        let mut g = self.lembrados.lock().unwrap_or_else(|p| p.into_inner());
        if g.len() >= TIPOS_LEMBRADOS {
            g.remove(0);
        }
        g.push(TipoLembrado {
            chave,
            tipo: tipo.clone(),
            por: por.clone(),
        });
        (tipo, por)
    }
}

impl Llm for LlmRoteado {
    fn id(&self) -> String {
        // O id volta a montagem (a retomada refaz o agente pelo `task.model`): tem de ser
        // um spec que a montagem entende.
        match &self.cabeca {
            Some(c) => format!("{SPEC_ROTA}:{c}"),
            None => SPEC_ROTA.to_string(),
        }
    }

    fn provedores(&self) -> Vec<String> {
        // Quem pode GASTAR numa chamada: a cabeca, todo provedor da politica (os montados)
        // e o modelo do classificador. Ele nao gera a resposta, mas cobra a conta da
        // tarefa: sem preco para ele, o orcamento em dinheiro nao se confere.
        let mut v: Vec<String> = self.modelos.keys().cloned().collect();
        if let Some(m) = self
            .politica
            .classificar
            .as_ref()
            .and_then(|c| c.modelo.as_ref())
            && !v.contains(m)
        {
            v.push(m.clone());
        }
        v
    }

    fn chat<'a>(
        &'a self,
        messages: &'a [Message],
        tools: &'a [ToolSpec],
        options: &'a LlmOptions,
    ) -> BoxFut<'a, Result<LlmReply, LlmError>> {
        Box::pin(async move {
            let (tipo, tipo_por) = self.tipo_de(messages).await;
            let cadeia = self
                .politica
                .cadeia(self.cabeca.as_deref(), tipo.as_deref());
            let mut a = Atendimento {
                tipo,
                tipo_por,
                cadeia: cadeia.clone(),
                tentativas: vec![],
                atendeu: None,
                parada: None,
            };
            let mut ultimo = LlmError::Denied("rota: cadeia vazia".into());
            for (i, spec) in cadeia.iter().enumerate() {
                let Some(llm) = self.modelos.get(spec) else {
                    // Impossivel pela montagem; dito em vez de pulado.
                    a.parada = Some(format!("{spec} nao foi montado"));
                    break;
                };
                let r = match self.politica.prazo {
                    Some(p) => tokio::time::timeout(p, llm.chat(messages, tools, options))
                        .await
                        .unwrap_or_else(|_| {
                            Err(LlmError::Transport(format!(
                                "prazo de {} s da politica de rota (timeout)",
                                p.as_secs()
                            )))
                        }),
                    None => llm.chat(messages, tools, options).await,
                };
                let e = match r {
                    Ok(resp) => {
                        a.tentativas.push(Tentativa {
                            spec: spec.clone(),
                            desfecho: "ok".into(),
                            erro: None,
                        });
                        a.atendeu = Some(spec.clone());
                        anotar(a);
                        return Ok(resp);
                    }
                    Err(e) => e,
                };
                let classe = classificar(&e);
                a.tentativas.push(Tentativa {
                    spec: spec.clone(),
                    desfecho: match classe {
                        Classe::Trocavel(f) => f.nome().into(),
                        Classe::NaoTroca(_) => "nao_troca".into(),
                    },
                    erro: Some(truncate_for_model(&e.to_string(), 300)),
                });
                ultimo = e;
                let parada = match classe {
                    Classe::NaoTroca(m) => Some(m.to_string()),
                    Classe::Trocavel(f) if !self.politica.trocar_em.contains(&f) => {
                        Some(format!("falha {} fora de 'trocar_em'", f.nome()))
                    }
                    Classe::Trocavel(_) if i + 1 >= cadeia.len() => {
                        Some("cadeia esgotada".to_string())
                    }
                    Classe::Trocavel(_) if a.trocas() >= self.politica.trocas_max as usize => {
                        Some(format!("teto de {} trocas", self.politica.trocas_max))
                    }
                    Classe::Trocavel(_) => None,
                };
                if parada.is_some() {
                    a.parada = parada;
                    break;
                }
            }
            anotar(a);
            Err(ultimo)
        })
    }
}

/// O modelo `rota`/`rota:<spec>` pela politica do `config.json`, com os provedores pela
/// MESMA fabrica da montagem (a chave sai do broker da raiz do agente).
pub fn do_config(
    cabeca: Option<&str>,
    raiz_do_agente: &std::path::Path,
) -> Result<Arc<dyn Llm>, String> {
    let arq = crate::config::caminho_de(CHAVE).ok_or_else(|| {
        format!(
            "o modelo `{SPEC_ROTA}` exige a politica de roteamento: defina {CHAVE} ({})",
            crate::config::variavel(CHAVE)
        )
    })?;
    let texto =
        std::fs::read_to_string(&arq).map_err(|e| format!("{CHAVE}: {}: {e}", arq.display()))?;
    let politica =
        Politica::de_json(&texto).map_err(|e| format!("{CHAVE}: {}: {e}", arq.display()))?;
    let raiz = raiz_do_agente.to_path_buf();
    let laya = || match crate::decisao::DecisorLaya::do_config(&raiz)? {
        Some((d, limiar)) => Ok(Degrau {
            decisor: Arc::new(d),
            limiar,
        }),
        None => Err(format!(
            "'classificar.laya' pede o decisor Laya: defina {} ({})",
            crate::decisao::laya::CHAVE_URL,
            crate::config::variavel(crate::decisao::laya::CHAVE_URL)
        )),
    };
    Ok(Arc::new(LlmRoteado::montar_com(
        politica,
        cabeca,
        |s| crate::chaves::modelo(s, &raiz),
        laya,
    )?))
}
