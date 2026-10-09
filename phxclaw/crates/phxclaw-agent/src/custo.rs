//! Custo em dinheiro de cada chamada ao modelo e de cada tarefa (R2 do radar de 09/10/2026).
//!
//! O preco e do OPERADOR, nunca do codigo: a tabela mora num arquivo dele
//! (`custo.precos`, `PHXCLAW_CUSTO_PRECOS`), com a moeda e, por modelo, entrada, saida e
//! cache por milhao de tokens, a data e a fonte da cotacao. Preco digitado no fonte
//! envelheceria calado -- o fornecedor muda a tabela e o binario continuaria jurando o
//! numero velho. Aqui, o relatorio diz de que dia e de onde veio cada preco.
//!
//! Decisoes que valem saber:
//! - **modelo sem preco e «nao medido», nunca zero.** Zero e o preco que o operador
//!   declarou (o Ollama local pode valer zero, se ele quiser); ausente e o que ninguem
//!   sabe. Basta UMA chamada sem preco para o total da tarefa ficar nao medido: somar so as
//!   que tem preco publicaria um total menor que o gasto.
//! - **o cache nao se aplica, e isso e dito.** Nenhum provedor deste agente devolve ao
//!   motor quantos tokens de entrada vieram do cache (`Usage` so tem entrada e saida); a
//!   entrada inteira e cobrada pelo preco cheio. O custo sai igual ou ACIMA do real, nunca
//!   abaixo -- que e o lado certo do erro para um orcamento.
//! - **a moeda e uma so por tabela.** Somar dolar com real seria numero sem unidade; o
//!   modelo que declarar outra moeda e recusado na leitura (`deny_unknown_fields`).

use phxclaw_agent_core::Usage;
use phxclaw_agent_core::tarefa::{Cotacao, CustoDaChamada, CustoDaTarefa, Task};
use phxclaw_evidence_ledger::{EvidenceDraft, EvidenceLedger, EvidenceOutcome};
use serde::Deserialize;
use serde_json::json;
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

/// Chave do arquivo da tabela de precos no catalogo (`PHXCLAW_CUSTO_PRECOS`).
pub const CHAVE_PRECOS: &str = "custo.precos";

/// Motivo de «nao medido» quando a instancia nao tem tabela nenhuma.
pub const SEM_TABELA: &str = "sem tabela de preços (custo.precos)";

/// O preco de um modelo, por milhao de tokens, com a cotacao de onde saiu.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Preco {
    pub entrada: f64,
    pub saida: f64,
    /// Preco da entrada lida do cache. Guardado e mostrado, mas nao aplicado: o provedor
    /// nao informa quantos tokens vieram do cache (ver o cabecalho).
    #[serde(default)]
    pub cache: Option<f64>,
    /// AAAA-MM-DD da cotacao.
    pub data: String,
    /// De onde o operador tirou o preco (URL da pagina de precos, contrato...).
    pub fonte: String,
}

/// A tabela do operador: uma moeda, e o preco de cada modelo pelo nome `provedor:modelo`
/// (o mesmo `id()` do provedor: `anthropic:claude-...`, `ollama:qwen2.5:1.5b`).
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TabelaDePrecos {
    pub moeda: String,
    pub modelos: BTreeMap<String, Preco>,
}

impl TabelaDePrecos {
    /// Le e confere. Tabela errada e erro dito com o modelo e o campo: tabela meio lida
    /// cobraria um modelo pelo preco certo e outro por nada.
    pub fn ler(arq: &Path) -> Result<Self, String> {
        let t = std::fs::read_to_string(arq).map_err(|e| format!("{}: {e}", arq.display()))?;
        Self::de_texto(&t).map_err(|e| format!("{}: {e}", arq.display()))
    }

    pub fn de_texto(t: &str) -> Result<Self, String> {
        let tab: TabelaDePrecos = serde_json::from_str(t).map_err(|e| e.to_string())?;
        tab.conferir()?;
        Ok(tab)
    }

    fn conferir(&self) -> Result<(), String> {
        let m = self.moeda.trim();
        if m.len() != 3 || !m.bytes().all(|b| b.is_ascii_uppercase()) {
            return Err(format!(
                "moeda «{}»: use o código ISO 4217 de três letras (USD, BRL, EUR)",
                self.moeda
            ));
        }
        if self.modelos.is_empty() {
            return Err("tabela sem nenhum modelo".into());
        }
        for (nome, p) in &self.modelos {
            if !nome.contains(':') {
                return Err(format!(
                    "modelo «{nome}»: use provedor:modelo, o mesmo nome do --modelo"
                ));
            }
            for (campo, v) in [
                ("entrada", Some(p.entrada)),
                ("saida", Some(p.saida)),
                ("cache", p.cache),
            ] {
                if let Some(v) = v
                    && !(v.is_finite() && v >= 0.0)
                {
                    return Err(format!("{nome}.{campo}: preço {v} não é um número >= 0"));
                }
            }
            if chrono::NaiveDate::parse_from_str(p.data.trim(), "%Y-%m-%d").is_err() {
                return Err(format!(
                    "{nome}.data «{}»: a data da cotação é AAAA-MM-DD",
                    p.data
                ));
            }
            if p.fonte.trim().is_empty() {
                return Err(format!(
                    "{nome}.fonte vazia: preço sem fonte é número que ninguém confere"
                ));
            }
        }
        Ok(())
    }

    pub fn preco(&self, modelo: &str) -> Option<&Preco> {
        self.modelos.get(modelo)
    }

    /// O custo de uma chamada, ou «nao medido» (`custo: None`) se o modelo nao tem preco.
    pub fn chamada(&self, modelo: &str, uso: &Usage) -> CustoDaChamada {
        let p = self.preco(modelo);
        CustoDaChamada {
            modelo: modelo.to_string(),
            tokens_entrada: uso.input_tokens,
            tokens_saida: uso.output_tokens,
            custo: p.map(|p| {
                (uso.input_tokens as f64 * p.entrada + uso.output_tokens as f64 * p.saida) / 1e6
            }),
            cotacao: p.map(|p| Cotacao {
                data: p.data.clone(),
                fonte: p.fonte.clone(),
            }),
        }
    }
}

/// A tabela da configuracao do processo. Chave definida com arquivo ruim e ERRO (a regra
/// do `config::configuracao`): seguir sem ela faria todo custo virar «nao medido» calado.
pub fn da_configuracao() -> Result<Option<Arc<TabelaDePrecos>>, String> {
    let Some(arq) = crate::config::texto(CHAVE_PRECOS)?.filter(|t| !t.trim().is_empty()) else {
        return Ok(None);
    };
    TabelaDePrecos::ler(Path::new(&arq))
        .map(|t| Some(Arc::new(t)))
        .map_err(|e| format!("{CHAVE_PRECOS}: {e}"))
}

/// Soma uma chamada ao custo da tarefa. Com tabela, a chamada entra na lista e na
/// evidencia; sem tabela, a tarefa so registra que o custo nao foi medido -- nenhuma
/// linha de evidencia nova, para quem nao configurou preco nao pagar por ele.
pub fn registrar(
    task: &mut Task,
    tabela: Option<&TabelaDePrecos>,
    modelo: &str,
    uso: &Usage,
    ledger: &EvidenceLedger,
) -> Option<CustoDaChamada> {
    let Some(tab) = tabela else {
        if task.custo.is_none() {
            task.custo = Some(CustoDaTarefa {
                nao_medido: Some(SEM_TABELA.into()),
                ..CustoDaTarefa::default()
            });
        }
        return None;
    };
    let ch = tab.chamada(modelo, uso);
    let c = task.custo.get_or_insert_with(|| CustoDaTarefa {
        moeda: Some(tab.moeda.clone()),
        total: Some(0.0),
        ..CustoDaTarefa::default()
    });
    // Tarefa que comecou sem tabela (retomada depois de o operador configurar) continua
    // nao medida: o comeco dela nao tem preco.
    c.total = match (c.total, ch.custo) {
        (Some(t), Some(x)) if c.nao_medido.is_none() => Some(t + x),
        _ => None,
    };
    if ch.custo.is_none() && c.nao_medido.is_none() {
        c.nao_medido = Some(format!("{modelo} sem preço na tabela"));
    }
    c.chamadas.push(ch.clone());
    let _ = ledger.append(EvidenceDraft {
        action_uuid: phxclaw_types::new_uuid_v7(),
        correlation_uuid: task.id.parse().ok(),
        actor: "phxclaw-agent".into(),
        capability: "llm.chat".into(),
        action: "custo.chamada".into(),
        outcome: EvidenceOutcome::Succeeded,
        request_summary: json!({"modelo": ch.modelo, "tokens_entrada": ch.tokens_entrada,
                                "tokens_saida": ch.tokens_saida}),
        result_summary: json!({"custo": ch.custo, "moeda": tab.moeda, "cotacao": ch.cotacao}),
        artifact_uris: vec![],
    });
    Some(ch)
}

/// O custo da tarefa COM as descendentes (o `gasto`), senao o das proprias chamadas;
/// `None` e «nao medido». E o numero do `/metrics` e da avaliacao.
pub fn total(t: &Task) -> Option<f64> {
    match &t.gasto {
        Some(g) => g.custo,
        None => t.custo.as_ref().and_then(|c| c.total),
    }
}

/// O custo em micro-dolares, so quando a tabela e em dolar: e o campo dos registros do
/// ai-benchmark e do arena (`actual_cost_micro_usd`), e converter moeda aqui seria inventar
/// cambio. Outra moeda fica «nao medido» ali, e o relatorio da tarefa diz a moeda certa.
pub fn micro_usd(t: &Task) -> Option<u64> {
    let moeda = t
        .gasto
        .as_ref()
        .and_then(|g| g.moeda.as_deref())
        .or_else(|| t.custo.as_ref().and_then(|c| c.moeda.as_deref()))?;
    (moeda == "USD")
        .then(|| total(t))
        .flatten()
        .map(|v| (v * 1e6).round() as u64)
}

/// A linha do relatorio da tarefa (CLI e tela): o total com a moeda e as cotacoes usadas,
/// ou «nao medido» com o motivo. Nunca «0» quando ninguem mediu.
pub fn linha(t: &Task) -> String {
    let proprio = match &t.custo {
        None => format!("não medido ({SEM_TABELA})"),
        Some(c) => match (c.total, &c.moeda) {
            (Some(v), Some(m)) => {
                let mut cot: Vec<String> = c
                    .chamadas
                    .iter()
                    .filter_map(|x| x.cotacao.as_ref())
                    .map(|q| format!("{} ({})", q.data, q.fonte))
                    .collect();
                cot.sort();
                cot.dedup();
                format!("{} {m}; cotação de {}", valor(v), cot.join(", "))
            }
            _ => format!(
                "não medido ({})",
                c.nao_medido.as_deref().unwrap_or(SEM_TABELA)
            ),
        },
    };
    let mut s = format!("custo: {proprio}");
    if let Some(g) = &t.gasto
        && g.tokens > t.usage.input_tokens + t.usage.output_tokens
    {
        s.push_str(&format!(
            "\ncom as tarefas filhas: {} tokens, {}",
            g.tokens,
            match (g.custo, &g.moeda) {
                (Some(v), Some(m)) => format!("{} {m}", valor(v)),
                _ => "custo não medido".into(),
            }
        ));
    }
    if let Some(o) = &t.orcamento {
        s.push_str(&format!("\norçamento: {}", descrever(o, moeda_de(t))));
    }
    s
}

fn moeda_de(t: &Task) -> Option<&str> {
    t.custo
        .as_ref()
        .and_then(|c| c.moeda.as_deref())
        .or_else(|| t.gasto.as_ref().and_then(|g| g.moeda.as_deref()))
}

/// O orcamento em texto, para a mensagem de parada e o relatorio.
pub fn descrever(o: &phxclaw_agent_core::tarefa::Orcamento, moeda: Option<&str>) -> String {
    let mut p = vec![];
    if let Some(t) = o.tokens {
        p.push(format!("{t} tokens"));
    }
    if let Some(c) = o.custo {
        p.push(format!(
            "{} {}",
            valor(c),
            moeda.unwrap_or("(moeda da tabela)")
        ));
    }
    if p.is_empty() {
        "sem teto".into()
    } else {
        p.join(" e ")
    }
}

/// Seis casas: o preco por chamada de um modelo barato e fracao de centavo, e arredondar
/// para dois digitos mostraria «0.00» -- o zero que este modulo existe para nao inventar.
pub fn valor(v: f64) -> String {
    format!("{v:.6}")
}

#[cfg(test)]
mod testes {
    use super::*;

    const TAB: &str = r#"{"moeda":"USD","modelos":{
        "anthropic:caro":{"entrada":3.0,"saida":15.0,"cache":0.3,"data":"2026-10-01","fonte":"https://exemplo/precos"},
        "ollama:local":{"entrada":0,"saida":0,"data":"2026-10-09","fonte":"maquina propria"}}}"#;

    fn uso(i: u64, o: u64) -> Usage {
        Usage {
            input_tokens: i,
            output_tokens: o,
            duracao_geracao_ns: None,
        }
    }

    fn ledger() -> EvidenceLedger {
        let d = std::env::temp_dir().join(format!("phx-custo-{}", uuid::Uuid::now_v7()));
        std::fs::create_dir_all(&d).unwrap();
        EvidenceLedger::open(d.join("ev.jsonl")).unwrap()
    }

    #[test]
    fn preco_por_milhao_e_zero_declarado_e_preco() {
        let t = TabelaDePrecos::de_texto(TAB).unwrap();
        let c = t.chamada("anthropic:caro", &uso(1_000_000, 100_000));
        assert_eq!(c.custo, Some(3.0 + 1.5));
        assert_eq!(c.cotacao.unwrap().data, "2026-10-01");
        // Zero escrito pelo operador e preco; nao e «nao medido».
        assert_eq!(t.chamada("ollama:local", &uso(500, 500)).custo, Some(0.0));
    }

    /// Guarda R2: modelo sem preco e «nao medido», nunca zero -- nem na chamada, nem no
    /// total da tarefa, mesmo quando as outras chamadas tem preco.
    #[test]
    fn modelo_sem_preco_e_nao_medido_nunca_zero() {
        let t = TabelaDePrecos::de_texto(TAB).unwrap();
        assert_eq!(t.chamada("openai:sem-preco", &uso(10, 10)).custo, None);
        let l = ledger();
        let mut task = Task::new("x", "anthropic:caro");
        registrar(&mut task, Some(&t), "anthropic:caro", &uso(1000, 1000), &l);
        assert!(task.custo.as_ref().unwrap().total.unwrap() > 0.0);
        registrar(&mut task, Some(&t), "openai:sem-preco", &uso(10, 10), &l);
        let c = task.custo.as_ref().unwrap();
        assert_eq!(c.total, None, "total com chamada sem preco: {c:?}");
        assert!(
            c.nao_medido
                .as_deref()
                .unwrap()
                .contains("openai:sem-preco")
        );
        assert!(linha(&task).contains("não medido"), "{}", linha(&task));
        // Sem tabela nenhuma: nao medido, e nenhuma evidencia de custo.
        let l2 = ledger();
        let mut sem = Task::new("x", "anthropic:caro");
        registrar(&mut sem, None, "anthropic:caro", &uso(1000, 1000), &l2);
        assert_eq!(sem.custo.as_ref().unwrap().total, None);
        assert_eq!(l2.verify().unwrap().records, 0);
        assert_eq!(l.verify().unwrap().records, 2);
    }

    #[test]
    fn tabela_errada_e_recusada_dizendo_o_campo() {
        let casos = [
            (
                r#"{"moeda":"dolar","modelos":{"a:b":{"entrada":1,"saida":1,"data":"2026-10-01","fonte":"f"}}}"#,
                "ISO 4217",
            ),
            (r#"{"moeda":"USD","modelos":{}}"#, "nenhum modelo"),
            (
                r#"{"moeda":"USD","modelos":{"semprovedor":{"entrada":1,"saida":1,"data":"2026-10-01","fonte":"f"}}}"#,
                "provedor:modelo",
            ),
            (
                r#"{"moeda":"USD","modelos":{"a:b":{"entrada":-1,"saida":1,"data":"2026-10-01","fonte":"f"}}}"#,
                "a:b.entrada",
            ),
            (
                r#"{"moeda":"USD","modelos":{"a:b":{"entrada":1,"saida":1,"data":"01/10/2026","fonte":"f"}}}"#,
                "AAAA-MM-DD",
            ),
            (
                r#"{"moeda":"USD","modelos":{"a:b":{"entrada":1,"saida":1,"data":"2026-10-01","fonte":" "}}}"#,
                "fonte",
            ),
            // Moeda por modelo nao existe: a tabela tem uma so.
            (
                r#"{"moeda":"USD","modelos":{"a:b":{"entrada":1,"saida":1,"data":"2026-10-01","fonte":"f","moeda":"BRL"}}}"#,
                "moeda",
            ),
        ];
        for (t, esperado) in casos {
            let e = TabelaDePrecos::de_texto(t).unwrap_err();
            assert!(e.contains(esperado), "{t}: {e}");
        }
    }
}
