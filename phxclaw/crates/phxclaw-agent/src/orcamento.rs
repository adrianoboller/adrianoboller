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
//! nunca segue calada. A conferencia vem DEPOIS da chamada que custou (o custo so se sabe
//! com a resposta) e ANTES da seguinte: o excesso maximo e uma chamada, e a mensagem diz
//! o numero real, nao o teto.
//!
//! Orcamento em dinheiro sem preco para o modelo nao se confere: a criacao o recusa, e o
//! motor para no primeiro gasto que nao tem preco (`custo` nao medido) em vez de seguir
//! gastando o que ninguem consegue somar.

use crate::custo::TabelaDePrecos;
use phxclaw_agent_core::tarefa::{Gasto, Orcamento};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};

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
}

fn contas() -> &'static Mutex<HashMap<String, Conta>> {
    static C: OnceLock<Mutex<HashMap<String, Conta>>> = OnceLock::new();
    C.get_or_init(|| Mutex::new(HashMap::new()))
}

static GERACAO: AtomicU64 = AtomicU64::new(1);

/// A conta aberta de uma tarefa em execucao; fechar e soltar (`Drop`).
#[derive(Debug)]
pub struct Aberta {
    id: String,
    geracao: u64,
}

/// A parada: o que bateu, dito em texto para o `error` da tarefa.
#[derive(Debug, Clone, PartialEq)]
pub struct Estouro(pub String);

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
        let g = trava();
        let mut atual = Some(self.id.clone());
        let mut passos = 0;
        while let Some(id) = atual.take() {
            passos += 1;
            let Some(c) = g.get(&id).filter(|_| passos <= 64) else {
                break;
            };
            if let Some(e) = bateu(&id, c, id == self.id) {
                return Err(e);
            }
            atual = c.pai.clone();
        }
        Ok(())
    }

    /// Cobra uma chamada desta tarefa nela e em todos os ancestrais abertos, e diz se
    /// alguem bateu. `custo` ausente e «nao medido» e contamina a soma para sempre.
    pub fn cobrar(
        &self,
        tokens: u64,
        custo: Option<f64>,
        moeda: Option<&str>,
    ) -> Result<(), Estouro> {
        let mut g = trava();
        let mut atual = Some(self.id.clone());
        let mut primeiro = None;
        let mut passos = 0;
        while let Some(id) = atual.take() {
            passos += 1;
            if passos > 64 {
                break;
            }
            let Some(c) = g.get_mut(&id) else { break };
            c.gasto.tokens = c.gasto.tokens.saturating_add(tokens);
            c.gasto.custo = match (c.gasto.custo, custo) {
                (Some(a), Some(b)) => Some(a + b),
                _ => None,
            };
            if c.gasto.moeda.is_none() {
                c.gasto.moeda = moeda.map(str::to_string);
            }
            if primeiro.is_none() {
                primeiro = bateu(&id, c, id == self.id);
            }
            atual = c.pai.clone();
        }
        primeiro.map_or(Ok(()), Err)
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
        },
    );
    Aberta {
        id: id.to_string(),
        geracao,
    }
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
