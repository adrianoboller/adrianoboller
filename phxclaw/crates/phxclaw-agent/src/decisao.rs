//! Decisao restrita (o «DecisionProvider» do radar R1): separar DECIDIR de GERAR.
//!
//! O provedor de geracao (`Llm`) devolve texto livre; uma decisao tem forma fechada e so
//! tres formas existem aqui -- `predicado` (sim/nao), `escolha` (uma de N opcoes fechadas) e
//! `nota` (0..1) --, sempre com a confianca ao lado. O que vale saber antes de mexer:
//!
//! - **Fora da forma e «sem decisao», nunca palpite.** Opcao que nao esta na lista, nota
//!   fora de 0..1, confianca ausente ou NaN: tudo vira `SemDecisao`. Aproximar («codigo»
//!   parece «código») seria o decisor inventando uma opcao que ninguem ofereceu. A
//!   conferencia mora em UMA funcao (`conferir`), pela qual passa a saida de todo decisor,
//!   inclusive o de fora desta crate: decisor que errasse a forma nao alcanca quem pergunta.
//! - **Barato primeiro, e o incerto sobe.** A `Escada` tenta os decisores em ordem (regras,
//!   depois modelo, depois modelo mais forte); abaixo do limiar do degrau, sobe; no fim,
//!   pessoa -- ou sem decisao, e quem perguntou fica com o comportamento de hoje.
//! - **A decisao nunca concede permissao** (lei C1 da cognicao: a rede so endurece). O
//!   UNICO caminho de uma decisao ate o portao de capacidades e `sobre_o_portao`, que so
//!   transforma `permitir` em `perguntar`/`negar`; a proposta que tentasse afrouxar e
//!   recusada com motivo e o veredito do portao fica intacto.
//! - **O Laya e um degrau, nao um atalho.** O `DecisorLaya` (`decisao/laya.rs`, cliente do
//!   `laya-serve`) e mais um `Decisor`: a saida dele passa pela mesma `conferir` e chega ao
//!   portao so por `sobre_o_portao`, como a de qualquer outro.

pub mod laya;
pub use laya::DecisorLaya;

use crate::regras::Decisao as NoPortao;
use phxclaw_agent_core::{BoxFut, Llm, LlmOptions, Message};
use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeSet;
use std::sync::Arc;

/// A forma da resposta que a pergunta aceita.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "forma", rename_all = "snake_case")]
pub enum Forma {
    Predicado,
    Escolha { opcoes: Vec<String> },
    Nota,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Questao {
    pub forma: Forma,
    /// O que se decide («tipo da tarefa», «este comando apaga dado?»).
    pub enunciado: String,
    /// O material sobre o qual se decide (o objetivo, a linha de comando).
    pub contexto: String,
}

impl Questao {
    pub fn predicado(enunciado: impl Into<String>, contexto: impl Into<String>) -> Self {
        Self {
            forma: Forma::Predicado,
            enunciado: enunciado.into(),
            contexto: contexto.into(),
        }
    }
    pub fn escolha(
        enunciado: impl Into<String>,
        opcoes: &[&str],
        contexto: impl Into<String>,
    ) -> Self {
        Self {
            forma: Forma::Escolha {
                opcoes: opcoes.iter().map(|s| s.to_string()).collect(),
            },
            enunciado: enunciado.into(),
            contexto: contexto.into(),
        }
    }
    pub fn nota(enunciado: impl Into<String>, contexto: impl Into<String>) -> Self {
        Self {
            forma: Forma::Nota,
            enunciado: enunciado.into(),
            contexto: contexto.into(),
        }
    }

    /// O texto que vai a pessoa quando a escada chega ao fim sem decisao confiante.
    pub fn para_pessoa(&self) -> String {
        match &self.forma {
            Forma::Predicado => format!("{} (sim/nao)", self.enunciado),
            Forma::Escolha { opcoes } => format!("{} [{}]", self.enunciado, opcoes.join(" / ")),
            Forma::Nota => format!("{} (numero de 0 a 1)", self.enunciado),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(untagged)]
pub enum Valor {
    Predicado(bool),
    Escolha(String),
    Nota(f64),
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Decidido {
    pub valor: Valor,
    /// 0..1, sempre presente: decisao sem confianca nao se distingue de palpite.
    pub confianca: f64,
    pub decisor: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "desfecho", rename_all = "snake_case")]
pub enum Desfecho {
    Decidido(Decidido),
    SemDecisao { decisor: String, motivo: String },
}

impl Desfecho {
    pub fn sem(decisor: impl Into<String>, motivo: impl Into<String>) -> Self {
        Desfecho::SemDecisao {
            decisor: decisor.into(),
            motivo: motivo.into(),
        }
    }
}

/// O provedor de decisao: uma interface separada do `Llm`, porque decidir e gerar tem
/// contratos diferentes -- quem gera pode errar a forma; quem decide nao pode.
pub trait Decisor: Send + Sync {
    fn id(&self) -> String;
    fn decidir<'a>(&'a self, q: &'a Questao) -> BoxFut<'a, Desfecho>;
}

fn confianca_valida(c: f64) -> bool {
    c.is_finite() && (0.0..=1.0).contains(&c)
}

/// A conferencia unica da forma. Toda saida de decisor passa aqui antes de chegar a quem
/// perguntou, e o que nao cabe na forma da questao vira `SemDecisao` dizendo o porque.
pub fn conferir(q: &Questao, d: Desfecho) -> Desfecho {
    let Desfecho::Decidido(x) = d else {
        return d;
    };
    if !confianca_valida(x.confianca) {
        return Desfecho::sem(
            x.decisor,
            format!("confianca fora de 0..1: {}", x.confianca),
        );
    }
    let erro = match (&q.forma, &x.valor) {
        (Forma::Predicado, Valor::Predicado(_)) => None,
        (Forma::Escolha { opcoes }, Valor::Escolha(v)) => {
            // Igualdade exata: aproximar seria inventar a opcao que ninguem ofereceu.
            (!opcoes.iter().any(|o| o == v)).then(|| format!("opcao fora da lista: {v:?}"))
        }
        (Forma::Nota, Valor::Nota(n)) => {
            (!confianca_valida(*n)).then(|| format!("nota fora de 0..1: {n}"))
        }
        (f, v) => Some(format!("valor {v:?} nao cabe na forma {f:?}")),
    };
    match erro {
        Some(m) => Desfecho::sem(x.decisor, m),
        None => Desfecho::Decidido(x),
    }
}

// ------------------------------------------------------------------ (a) regras

/// Regra deterministica: se o contexto contem alguma das palavras, o valor e este.
#[derive(Debug, Clone, PartialEq)]
pub struct RegraDeDecisao {
    pub palavras: Vec<String>,
    pub valor: Valor,
    pub confianca: f64,
}

impl RegraDeDecisao {
    /// `{"palavras": ["cargo", "rust"], "valor": "codigo", "confianca": 0.9}`. O tipo do
    /// `valor` diz a forma: booleano e predicado, texto e escolha, numero e nota.
    pub fn de_json(v: &Value) -> Result<Self, String> {
        let palavras: Vec<String> = v
            .get("palavras")
            .and_then(Value::as_array)
            .ok_or("regra sem 'palavras'")?
            .iter()
            .map(|p| p.as_str().map(crate::equipe::dobrar))
            .collect::<Option<_>>()
            .ok_or("'palavras' tem de ser lista de textos")?;
        if palavras.is_empty() || palavras.iter().any(String::is_empty) {
            return Err("'palavras' vazia ou com palavra vazia".into());
        }
        let valor = match v.get("valor") {
            Some(Value::Bool(b)) => Valor::Predicado(*b),
            Some(Value::String(s)) => Valor::Escolha(s.clone()),
            Some(Value::Number(n)) => Valor::Nota(n.as_f64().unwrap_or(f64::NAN)),
            _ => return Err("regra sem 'valor' (booleano, texto ou numero)".into()),
        };
        let confianca = v
            .get("confianca")
            .and_then(Value::as_f64)
            .ok_or("regra sem 'confianca'")?;
        if !confianca_valida(confianca) {
            return Err(format!("confianca fora de 0..1: {confianca}"));
        }
        Ok(Self {
            palavras,
            valor,
            confianca,
        })
    }
}

/// O decisor mais barato: palavras no contexto. Duas regras casando com valores
/// diferentes e conflito, e conflito e `SemDecisao` -- escolher uma das duas seria a
/// ordem do arquivo decidindo calada.
pub struct DecisorDeRegras {
    pub nome: String,
    pub regras: Vec<RegraDeDecisao>,
}

impl Decisor for DecisorDeRegras {
    fn id(&self) -> String {
        format!("regras:{}", self.nome)
    }
    fn decidir<'a>(&'a self, q: &'a Questao) -> BoxFut<'a, Desfecho> {
        Box::pin(async move {
            let texto = crate::equipe::dobrar(&q.contexto);
            let casadas: Vec<&RegraDeDecisao> = self
                .regras
                .iter()
                .filter(|r| r.palavras.iter().any(|p| texto.contains(p.as_str())))
                .collect();
            let Some(primeira) = casadas.first() else {
                return Desfecho::sem(self.id(), "nenhuma regra casou");
            };
            if casadas.iter().any(|r| r.valor != primeira.valor) {
                let valores: Vec<String> =
                    casadas.iter().map(|r| format!("{:?}", r.valor)).collect();
                return Desfecho::sem(
                    self.id(),
                    format!("regras em conflito: {}", valores.join(", ")),
                );
            }
            let confianca = casadas.iter().map(|r| r.confianca).fold(0.0, f64::max);
            conferir(
                q,
                Desfecho::Decidido(Decidido {
                    valor: primeira.valor.clone(),
                    confianca,
                    decisor: self.id(),
                }),
            )
        })
    }
}

// ------------------------------------------------------------------ (b) modelo restrito

/// Um provedor de geracao usado em modo restrito: pede JSON `{"value", "confidence"}`,
/// sem ferramentas, temperatura 0 e teto curto, e passa o que voltou pela `conferir`.
/// Erro do provedor, JSON que nao se le e `value: null` sao `SemDecisao` -- o decisor
/// nao tem como errar para o lado do palpite.
pub struct DecisorPorModelo {
    pub llm: Arc<dyn Llm>,
}

const PROMPT_DECISOR: &str = "You are a strict decision function, not an assistant. \
Answer ONLY with one JSON object: {\"value\": <answer>, \"confidence\": <number from 0 to 1>}. \
No prose, no code fences. If you cannot decide, answer {\"value\": null, \"confidence\": 0}.";

impl DecisorPorModelo {
    fn pedido(q: &Questao) -> String {
        let forma = match &q.forma {
            Forma::Predicado => "value must be true or false (JSON boolean).".to_string(),
            Forma::Escolha { opcoes } => format!(
                "value must be EXACTLY one of these strings, copied verbatim: {}.",
                serde_json::to_string(opcoes).unwrap_or_default()
            ),
            Forma::Nota => "value must be a number from 0 to 1.".to_string(),
        };
        format!(
            "Question: {}\n{forma}\n\n<input>\n{}\n</input>",
            q.enunciado, q.contexto
        )
    }

    /// O JSON da resposta: o objeto inteiro, ou o primeiro `{...}` do texto (modelo
    /// pequeno cerca com crase mesmo pedido para nao cercar).
    fn ler(texto: &str) -> Option<Value> {
        let t = texto.trim();
        if let Ok(v) = serde_json::from_str::<Value>(t) {
            return Some(v);
        }
        let (i, f) = (t.find('{')?, t.rfind('}')?);
        (i < f)
            .then(|| serde_json::from_str(&t[i..=f]).ok())
            .flatten()
    }
}

impl Decisor for DecisorPorModelo {
    fn id(&self) -> String {
        format!("modelo:{}", self.llm.id())
    }
    fn decidir<'a>(&'a self, q: &'a Questao) -> BoxFut<'a, Desfecho> {
        Box::pin(async move {
            let msgs = [
                Message::system(PROMPT_DECISOR),
                Message::user(Self::pedido(q)),
            ];
            let opcoes = LlmOptions {
                max_output_tokens: 128,
                temperature: 0.0,
            };
            let r = match self.llm.chat(&msgs, &[], &opcoes).await {
                Ok(r) => r,
                Err(e) => return Desfecho::sem(self.id(), format!("provedor: {e}")),
            };
            let Some(v) = Self::ler(&r.content) else {
                return Desfecho::sem(self.id(), "resposta sem JSON legivel");
            };
            let Some(confianca) = v.get("confidence").and_then(Value::as_f64) else {
                return Desfecho::sem(self.id(), "resposta sem 'confidence' numerica");
            };
            let valor = match v.get("value") {
                Some(Value::Bool(b)) => Valor::Predicado(*b),
                Some(Value::String(s)) => Valor::Escolha(s.clone()),
                Some(Value::Number(n)) => Valor::Nota(n.as_f64().unwrap_or(f64::NAN)),
                Some(Value::Null) | None => {
                    return Desfecho::sem(self.id(), "o modelo declinou de decidir");
                }
                Some(outro) => {
                    return Desfecho::sem(self.id(), format!("'value' de tipo invalido: {outro}"));
                }
            };
            conferir(
                q,
                Desfecho::Decidido(Decidido {
                    valor,
                    confianca,
                    decisor: self.id(),
                }),
            )
        })
    }
}

// ------------------------------------------------------------------ a escada

pub struct Degrau {
    pub decisor: Arc<dyn Decisor>,
    /// Confianca minima para a decisao deste degrau valer; abaixo, sobe.
    pub limiar: f64,
}

/// Como a escada terminou.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "fim", rename_all = "snake_case")]
pub enum Encaminhamento {
    Decidido(Decidido),
    /// Ninguem decidiu com confianca e ha uma pessoa no fim: quem perguntou faz a
    /// pergunta (o `ask_user`/`perguntar` do motor) e le a resposta por `resposta_da_pessoa`.
    Pessoa {
        pergunta: String,
    },
    /// Ninguem decidiu e nao ha pessoa: vale o comportamento de hoje de quem perguntou.
    SemDecisao,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Resultado {
    pub fim: Encaminhamento,
    /// Cada degrau tentado e o que devolveu, para a evidencia dizer por que subiu.
    pub trilha: Vec<Desfecho>,
}

pub struct Escada {
    pub degraus: Vec<Degrau>,
    /// Com pessoa, o fim sem decisao vira pergunta; sem, vira `SemDecisao`.
    pub pessoa: bool,
}

impl Escada {
    pub async fn decidir(&self, q: &Questao) -> Resultado {
        let mut trilha = Vec::new();
        for d in &self.degraus {
            // `conferir` de novo: o decisor pode ser de fora desta crate.
            let r = conferir(q, d.decisor.decidir(q).await);
            trilha.push(r.clone());
            if let Desfecho::Decidido(x) = r
                && x.confianca >= d.limiar
            {
                return Resultado {
                    fim: Encaminhamento::Decidido(x),
                    trilha,
                };
            }
        }
        let fim = if self.pessoa {
            Encaminhamento::Pessoa {
                pergunta: q.para_pessoa(),
            }
        } else {
            Encaminhamento::SemDecisao
        };
        Resultado { fim, trilha }
    }
}

/// A resposta da pessoa, pela MESMA conferencia de forma: opcao que nao esta na lista
/// continua sendo `SemDecisao`, e a pessoa decide com confianca 1.
pub fn resposta_da_pessoa(q: &Questao, texto: &str) -> Desfecho {
    let t = texto.trim();
    let valor = match &q.forma {
        Forma::Predicado => {
            let primeira = crate::equipe::dobrar(t)
                .split(|c: char| !c.is_alphanumeric())
                .find(|p| !p.is_empty())
                .unwrap_or("")
                .to_string();
            if crate::perguntas::afirmativa(t) {
                Some(Valor::Predicado(true))
            } else if matches!(primeira.as_str(), "nao" | "n" | "no" | "negar" | "nego") {
                Some(Valor::Predicado(false))
            } else {
                None
            }
        }
        Forma::Escolha { opcoes } => {
            let pelo_numero = t
                .parse::<usize>()
                .ok()
                .and_then(|i| i.checked_sub(1))
                .and_then(|i| opcoes.get(i));
            pelo_numero
                .or_else(|| {
                    opcoes
                        .iter()
                        .find(|o| crate::equipe::dobrar(o) == crate::equipe::dobrar(t))
                })
                .map(|o| Valor::Escolha(o.clone()))
        }
        Forma::Nota => t.replace(',', ".").parse::<f64>().ok().map(Valor::Nota),
    };
    match valor {
        Some(valor) => conferir(
            q,
            Desfecho::Decidido(Decidido {
                valor,
                confianca: 1.0,
                decisor: "pessoa".into(),
            }),
        ),
        None => Desfecho::sem("pessoa", format!("resposta fora da forma: {t:?}")),
    }
}

// ------------------------------------------------------------------ perto do portao

/// O que uma escada propoe ao portao. Decidido «sim» num predicado de cuidado e pessoa
/// no fim viram `perguntar`; uma escolha com as opcoes `permitir`/`perguntar`/`negar`
/// vale pelo nome -- inclusive `permitir`, que e justamente a proposta que o portao tem
/// de recusar quando a base e mais estrita. Sem decisao, ou decisao que nao fala do
/// portao, nao propoe nada (`None`): vale o comportamento de hoje.
pub fn proposta(fim: &Encaminhamento) -> Option<NoPortao> {
    match fim {
        Encaminhamento::Pessoa { .. } => Some(NoPortao::Perguntar),
        Encaminhamento::SemDecisao => None,
        Encaminhamento::Decidido(d) => match &d.valor {
            Valor::Predicado(true) => Some(NoPortao::Perguntar),
            Valor::Escolha(s) => match s.as_str() {
                "negar" => Some(NoPortao::Negar),
                "perguntar" => Some(NoPortao::Perguntar),
                "permitir" => Some(NoPortao::Permitir),
                _ => None,
            },
            _ => None,
        },
    }
}

/// O veredito do portao depois da decisao, e a recusa quando a decisao tentou afrouxar.
#[derive(Debug, Clone, PartialEq)]
pub struct VereditoEndurecido {
    pub decisao: NoPortao,
    pub recusa: Option<String>,
}

/// O UNICO caminho de uma decisao ate o portao. `base` e o que o portao ja decidiu sem
/// ela (capacidade nao concedida e `negar`); o resultado e o mais estrito dos dois. A
/// proposta mais frouxa que a base e recusada e registrada -- nunca aplicada.
pub fn sobre_o_portao(base: NoPortao, proposta: NoPortao) -> VereditoEndurecido {
    let recusa = (proposta < base).then(|| {
        format!(
            "decisao recusada: propunha {proposta:?} sobre {base:?}; decisao nao concede permissao"
        )
    });
    VereditoEndurecido {
        decisao: base.max(proposta),
        recusa,
    }
}

/// A capacidade sob uma decisao: nao concedida e `negar` antes de qualquer decisao, e a
/// regra de comando (quando ha) entra como base -- a decisao so pode endurecer as duas.
pub fn capacidade_sob_decisao(
    concedidas: &BTreeSet<String>,
    capacidade: &str,
    regra: Option<NoPortao>,
    fim: &Encaminhamento,
) -> VereditoEndurecido {
    let base = if concedidas.contains(capacidade) {
        regra.unwrap_or(NoPortao::Permitir)
    } else {
        NoPortao::Negar
    };
    match proposta(fim) {
        Some(p) => sobre_o_portao(base, p),
        None => VereditoEndurecido {
            decisao: base,
            recusa: None,
        },
    }
}
