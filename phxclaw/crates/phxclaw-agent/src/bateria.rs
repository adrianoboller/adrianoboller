//! Bateria comum entre provedores (R5 do radar de 09/10/2026): `phxclaw avaliar
//! --provedores A,B --bateria ARQ`. A MESMA lista de casos, com o MESMO gabarito, rodada por
//! cada provedor pelo caminho de verdade do agente (`avaliacao::executar_caso`: motor,
//! portao, ferramentas, gravacao), medindo cinco coisas:
//!
//! - **acerto**: a fracao dos ensaios que passou no gabarito;
//! - **custo por acerto**: o custo de todas as tentativas, inclusive as que falharam, sobre
//!   os acertos (R2) -- nao medido se algum provedor nao tem preco na tabela;
//! - **duracao**: a soma das tentativas do ensaio, em ms;
//! - **tentativas**: quantas o ensaio usou ate acertar (ou o teto);
//! - **intervencao**: a fracao dos ensaios em que o agente pediu uma pessoa (`ask_user`, ou
//!   regra `perguntar`). A bateria roda sem ninguem: o prazo de resposta e zero, a pergunta
//!   e contada e o agente segue pelo proprio juizo.
//!
//! **Vencedor so quando as faixas nao se cruzam**, e a faixa e o intervalo de 95% por
//! bootstrap (reamostra os CASOS com reposicao, os mesmos indices para todos os
//! provedores -- e a comparacao pareada). O juiz e o `avaliacao::vencedor_entre`, o mesmo
//! do `avaliar`: mediana melhor dentro do ruido nao e vitoria.
//!
//! O `phxclaw-model-arena` entra como REGISTRO: o primeiro provedor e o campeao, os outros
//! desafiantes, e cada par (caso, rodada) vira uma `ArenaPairObservation`, agregada na
//! janela com hash do arena. O veredito dele (diferenca de media) vai no relatorio como
//! informativo e NUNCA decide: e a regra que as faixas recusariam. Evidencia `Fixture`: o
//! arena recusa promover perfil a partir dela, e e o certo -- a bateria mede, nao troca o
//! modelo de ninguem.
//!
//! Provedor que nao respondeu a nenhuma chamada (sem chave, servidor fora do ar) sai como
//! NAO MEDIDO, com o primeiro erro, e fica fora do vencedor: «nao respondeu» nao e «errou».

use crate::avaliacao::{
    Caso, Execucao, Fabrica, Faixa, LeitorEnergia, Medida, Vencedor, executar_caso, ler_casos,
    nome_de_arquivo, vencedor_entre,
};
use crate::motor::{Observer, sha256_hex};
use crate::tarefa::{Task, TaskStatus};
use phxclaw_model_arena as arena;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

fn tres() -> usize {
    3
}
fn um() -> usize {
    1
}
fn mil() -> usize {
    1_000
}
fn semente_padrao() -> u64 {
    0x5048_5843_4c41_5735 // "PHXCLAW5": fixa, para a mesma bateria dar o mesmo intervalo
}

/// O arquivo da bateria. `casos` e o gabarito fixo; o resto tem padrao.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Bateria {
    pub casos: Vec<Caso>,
    #[serde(default = "tres")]
    pub rodadas: usize,
    /// Teto de tentativas por ensaio: tenta de novo so o que errou.
    #[serde(default = "um")]
    pub tentativas: usize,
    #[serde(default = "mil")]
    pub reamostras: usize,
    #[serde(default = "semente_padrao")]
    pub semente: u64,
}

/// Le a bateria: um arquivo JSON (`{"casos": [...], "rodadas": N, ...}`) ou uma pasta de
/// casos do `avaliar` (`*.json`/`*.jsonl`), com os padroes.
pub fn ler(arq: &Path) -> Result<Bateria, String> {
    let b = if arq.is_dir() {
        Bateria {
            casos: ler_casos(arq)?,
            rodadas: tres(),
            tentativas: um(),
            reamostras: mil(),
            semente: semente_padrao(),
        }
    } else {
        let t = std::fs::read_to_string(arq).map_err(|e| format!("{}: {e}", arq.display()))?;
        serde_json::from_str(&t).map_err(|e| format!("{}: {e}", arq.display()))?
    };
    conferir(&b).map_err(|e| format!("{}: {e}", arq.display()))?;
    Ok(b)
}

fn conferir(b: &Bateria) -> Result<(), String> {
    if b.casos.is_empty() {
        return Err("bateria sem casos".into());
    }
    let mut ids = std::collections::BTreeSet::new();
    for c in &b.casos {
        if c.id.trim().is_empty() || !ids.insert(c.id.as_str()) {
            return Err(format!("caso com id vazio ou repetido: «{}»", c.id));
        }
        if c.gabarito.contem.is_empty() && c.gabarito.sequencia.is_none() {
            return Err(format!("{}: caso sem gabarito (contem ou sequencia)", c.id));
        }
    }
    if !(1..=100).contains(&b.rodadas) {
        return Err(format!("rodadas {}: de 1 a 100", b.rodadas));
    }
    if !(1..=10).contains(&b.tentativas) {
        return Err(format!("tentativas {}: de 1 a 10", b.tentativas));
    }
    if !(200..=100_000).contains(&b.reamostras) {
        return Err(format!(
            "reamostras {}: de 200 a 100000 (menos que isso, a ponta de 2,5% e meia duzia de pontos)",
            b.reamostras
        ));
    }
    Ok(())
}

/// Um ensaio: um caso, numa rodada, por um provedor, com as tentativas que usou.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Ensaio {
    pub provedor: String,
    pub caso: String,
    pub rodada: usize,
    pub acerto: bool,
    pub tentativas: usize,
    /// Perguntas a uma pessoa nas tentativas (zero = rodou sozinho).
    pub intervencoes: usize,
    /// Soma das tentativas; ausente = alguma sem custo medido.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custo: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custo_micro_usd: Option<u64>,
    pub duracao_ms: f64,
    pub tokens: u64,
    pub respostas_modelo: usize,
    pub estados: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub erro: Option<String>,
    pub resposta_sha256: String,
    pub gravacoes: Vec<PathBuf>,
}

/// As cinco metricas de um provedor, cada uma com o intervalo de 95% por bootstrap
/// (`min` e `max` da `Faixa` sao as pontas; `mediana` e a estimativa na amostra inteira;
/// `n` e o numero de casos, a unidade reamostrada).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PorProvedor {
    pub provedor: String,
    pub ensaios: usize,
    /// Presente quando o provedor nao respondeu a nenhuma chamada: fora do vencedor.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nao_medido: Option<String>,
    pub acerto: Medida,
    pub custo_por_acerto: Medida,
    pub duracao_ms: Medida,
    pub tentativas: Medida,
    pub intervencao: Medida,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JanelaDoArena {
    pub campeao: String,
    pub desafiante: String,
    pub janela: arena::ArenaWindow,
    /// Informativo: o arena decide por diferenca de media, e aqui quem decide sao as faixas.
    pub veredito_do_arena: arena::ArenaVerdict,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResultadoBateria {
    pub data: String,
    pub casos: usize,
    pub rodadas: usize,
    pub tentativas: usize,
    pub reamostras: usize,
    pub semente: u64,
    pub provedores: Vec<PorProvedor>,
    pub vencedores: Vec<Vencedor>,
    pub arena: Vec<JanelaDoArena>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arena_erro: Option<String>,
    pub ensaios: Vec<Ensaio>,
}

/// Conta as perguntas a uma pessoa: cada pergunta grava a tarefa UMA vez em
/// `AwaitingInput` (o `perguntar` do motor), e e isso que se conta.
#[derive(Default)]
struct ContaPerguntas(AtomicUsize);

impl Observer for ContaPerguntas {
    fn on_update(&self, task: &Task) {
        if task.status == TaskStatus::AwaitingInput {
            self.0.fetch_add(1, Ordering::Relaxed);
        }
    }
}

/// Roda a bateria. Um ensaio por vez, pelo mesmo motivo do `avaliar`: em paralelo, a
/// duracao mediria a fila.
pub async fn rodar(
    fabrica: Fabrica<'_>,
    provedores: &[String],
    b: &Bateria,
    saida: &Path,
    energia: &LeitorEnergia,
) -> Result<ResultadoBateria, String> {
    conferir(b)?;
    if provedores.is_empty() {
        return Err("bateria pede ao menos um provedor".into());
    }
    let mut vistos = std::collections::BTreeSet::new();
    if let Some(r) = provedores.iter().find(|p| !vistos.insert(p.as_str())) {
        return Err(format!("provedor repetido: {r}"));
    }
    let dir = saida.join("gravacoes");
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    // Sem ninguem para responder: a pergunta e contada e o agente segue na hora.
    let sem_gente = |modelo: &str| {
        fabrica(modelo).map(|mut a| {
            a.config.prazo_de_resposta = Some(Duration::ZERO);
            a
        })
    };
    let mut ensaios = Vec::new();
    for p in provedores {
        for rodada in 1..=b.rodadas {
            for caso in &b.casos {
                ensaios
                    .push(ensaio(&sem_gente, p, caso, rodada, b.tentativas, &dir, energia).await?);
            }
        }
    }
    Ok(resumir(provedores, b, ensaios))
}

async fn ensaio(
    fabrica: Fabrica<'_>,
    provedor: &str,
    caso: &Caso,
    rodada: usize,
    teto: usize,
    dir: &Path,
    energia: &LeitorEnergia,
) -> Result<Ensaio, String> {
    let mut e = Ensaio {
        provedor: provedor.to_string(),
        caso: caso.id.clone(),
        rodada,
        acerto: false,
        tentativas: 0,
        intervencoes: 0,
        custo: Some(0.0),
        custo_micro_usd: Some(0),
        duracao_ms: 0.0,
        tokens: 0,
        respostas_modelo: 0,
        estados: vec![],
        erro: None,
        resposta_sha256: sha256_hex(b""),
        gravacoes: vec![],
    };
    for k in 1..=teto {
        let arq = dir.join(format!(
            "{}-{}-r{rodada}-t{k}.jsonl",
            nome_de_arquivo(provedor),
            nome_de_arquivo(&caso.id)
        ));
        let perguntas = ContaPerguntas::default();
        let (ex, t): (Execucao, Task) =
            executar_caso(fabrica, provedor, caso, rodada, &arq, energia, &perguntas).await?;
        e.tentativas = k;
        e.intervencoes += perguntas.0.load(Ordering::Relaxed);
        // A tentativa que falhou tambem foi paga: entra na soma (R2).
        e.custo = e.custo.zip(ex.custo).map(|(a, b)| a + b);
        e.custo_micro_usd = e
            .custo_micro_usd
            .zip(crate::custo::micro_usd(&t))
            .map(|(a, b)| a + b);
        e.duracao_ms += ex.latencia_ms;
        e.tokens += t.usage.input_tokens + t.usage.output_tokens;
        e.respostas_modelo += ex.respostas_modelo;
        e.estados.push(ex.estado.clone());
        e.erro = ex.erro.clone();
        e.resposta_sha256 = sha256_hex(t.answer.unwrap_or_default().as_bytes());
        e.gravacoes.push(arq);
        if ex.acerto {
            e.acerto = true;
            break;
        }
    }
    Ok(e)
}

// ------------------------------------------------------------------ bootstrap

/// SplitMix64: o sorteio do bootstrap sem crate nova, deterministico pela semente --
/// a mesma bateria com a mesma semente da o mesmo intervalo, e quem refaz confere.
struct Sorteio(u64);

impl Sorteio {
    fn proximo(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    fn ate(&mut self, n: usize) -> usize {
        (self.proximo() % n as u64) as usize
    }
}

/// O que um caso contribui, ja somado nas rodadas.
#[derive(Debug, Clone, Default)]
struct PorCaso {
    acerto: f64,
    acertos: f64,
    custo: Option<f64>,
    duracao: f64,
    tentativas: f64,
    intervencao: f64,
}

fn por_caso(ens: &[&Ensaio]) -> PorCaso {
    let n = ens.len().max(1) as f64;
    PorCaso {
        acerto: ens.iter().filter(|e| e.acerto).count() as f64 / n,
        acertos: ens.iter().filter(|e| e.acerto).count() as f64,
        custo: ens.iter().map(|e| e.custo).sum::<Option<f64>>(),
        duracao: ens.iter().map(|e| e.duracao_ms).sum::<f64>() / n,
        tentativas: ens.iter().map(|e| e.tentativas as f64).sum::<f64>() / n,
        intervencao: ens.iter().filter(|e| e.intervencoes > 0).count() as f64 / n,
    }
}

type Estatistica = fn(&[&PorCaso]) -> Option<f64>;

fn media(v: &[&PorCaso], f: fn(&PorCaso) -> f64) -> Option<f64> {
    (!v.is_empty()).then(|| v.iter().map(|c| f(c)).sum::<f64>() / v.len() as f64)
}

/// As cinco estatisticas, com o nome, o lado bom e a funcao sobre uma amostra de casos.
const ESTATISTICAS: [(&str, bool, Estatistica); 5] = [
    ("acerto", true, |v| media(v, |c| c.acerto)),
    ("custo_por_acerto", false, |v| {
        // Razao das somas: o custo de TODAS as tentativas sobre os acertos.
        let custo = v.iter().map(|c| c.custo).sum::<Option<f64>>()?;
        let acertos: f64 = v.iter().map(|c| c.acertos).sum();
        (acertos > 0.0).then(|| custo / acertos)
    }),
    ("duracao_ms", false, |v| media(v, |c| c.duracao)),
    ("tentativas", false, |v| media(v, |c| c.tentativas)),
    ("intervencao", false, |v| media(v, |c| c.intervencao)),
];

/// Posto mais proximo em milesimos (25 = 2,5%): o `percentil` do `avaliar` e em inteiros.
fn ponta(s: &[f64], mil: usize) -> f64 {
    let i = (s.len() * mil)
        .div_ceil(1000)
        .saturating_sub(1)
        .min(s.len() - 1);
    s[i]
}

/// O intervalo de 95% de cada estatistica, pelos MESMOS indices reamostrados para todos os
/// provedores (pareado). Estatistica sem valor em alguma reamostra (custo nao medido, ou
/// reamostra sem acerto) sai nao medida, com o motivo: ponta infinita nao e numero.
fn intervalos(casos: &[PorCaso], indices: &[Vec<usize>]) -> Vec<Medida> {
    let todos: Vec<&PorCaso> = casos.iter().collect();
    ESTATISTICAS
        .iter()
        .map(|(nome, _, est)| {
            let Some(ponto) = est(&todos) else {
                return Medida::NaoMedida(motivo(nome, casos));
            };
            let mut v = Vec::with_capacity(indices.len());
            for idx in indices {
                let amostra: Vec<&PorCaso> = idx.iter().map(|i| &casos[*i]).collect();
                match est(&amostra) {
                    Some(x) => v.push(x),
                    None => {
                        return Medida::NaoMedida(format!(
                            "{}: há reamostra sem nenhum acerto, e o intervalo não tem teto",
                            nome
                        ));
                    }
                }
            }
            v.sort_by(f64::total_cmp);
            Medida::Faixa(Faixa {
                n: casos.len(),
                min: ponta(&v, 25),
                mediana: ponto,
                max: ponta(&v, 975),
            })
        })
        .collect()
}

fn motivo(nome: &str, casos: &[PorCaso]) -> String {
    if nome == "custo_por_acerto" {
        if casos.iter().any(|c| c.custo.is_none()) {
            "custo não medido (provedor sem preço na tabela custo.precos)".into()
        } else {
            "nenhum acerto: sem denominador".into()
        }
    } else {
        "sem casos".into()
    }
}

fn resumir(provedores: &[String], b: &Bateria, ensaios: Vec<Ensaio>) -> ResultadoBateria {
    let mut s = Sorteio(b.semente);
    let n = b.casos.len();
    let indices: Vec<Vec<usize>> = (0..b.reamostras)
        .map(|_| (0..n).map(|_| s.ate(n)).collect())
        .collect();
    let mut res = Vec::new();
    for p in provedores {
        let meus: Vec<&Ensaio> = ensaios.iter().filter(|e| &e.provedor == p).collect();
        let casos: Vec<PorCaso> = b
            .casos
            .iter()
            .map(|c| {
                por_caso(
                    &meus
                        .iter()
                        .copied()
                        .filter(|e| e.caso == c.id)
                        .collect::<Vec<_>>(),
                )
            })
            .collect();
        let mut m = intervalos(&casos, &indices).into_iter();
        let mut prox = || m.next().expect("cinco estatisticas");
        let nao_medido = (meus.iter().all(|e| e.respostas_modelo == 0)).then(|| {
            format!(
                "NÃO MEDIDO: o provedor não respondeu a nenhuma chamada ({})",
                meus.iter()
                    .find_map(|e| e.erro.clone())
                    .unwrap_or_else(|| "sem erro registrado".into())
            )
        });
        res.push(PorProvedor {
            provedor: p.clone(),
            ensaios: meus.len(),
            nao_medido,
            acerto: prox(),
            custo_por_acerto: prox(),
            duracao_ms: prox(),
            tentativas: prox(),
            intervencao: prox(),
        });
    }
    let vencedores = decidir(&res);
    let (arena, arena_erro) = match registro_do_arena(provedores, b, &ensaios) {
        Ok(v) => (v, None),
        Err(e) => (vec![], Some(e)),
    };
    ResultadoBateria {
        data: chrono::Utc::now().to_rfc3339(),
        casos: n,
        rodadas: b.rodadas,
        tentativas: b.tentativas,
        reamostras: b.reamostras,
        semente: b.semente,
        provedores: res,
        vencedores,
        arena,
        arena_erro,
        ensaios,
    }
}

fn faixa_de(p: &PorProvedor, nome: &str) -> Option<Faixa> {
    match nome {
        "acerto" => p.acerto.faixa().copied(),
        "custo_por_acerto" => p.custo_por_acerto.faixa().copied(),
        "duracao_ms" => p.duracao_ms.faixa().copied(),
        "tentativas" => p.tentativas.faixa().copied(),
        _ => p.intervencao.faixa().copied(),
    }
}

/// Para cada metrica: vence quem tem a faixa separada da de todos os outros medidos, do
/// lado bom (`vencedor_entre`, o juiz do `avaliar`). Provedor nao medido fica fora, e a
/// metrica nao medida em algum dos medidos fica sem vencedor, dito.
pub fn decidir(r: &[PorProvedor]) -> Vec<Vencedor> {
    let medidos: Vec<&PorProvedor> = r.iter().filter(|p| p.nao_medido.is_none()).collect();
    let fora = r.len() - medidos.len();
    let nota = if fora > 0 {
        format!("; {fora} provedor(es) não medido(s) fora da comparação")
    } else {
        String::new()
    };
    ESTATISTICAS
        .iter()
        .map(|(nome, maior, _)| {
            let (modelo, motivo) = if medidos.len() < 2 {
                (None, format!("menos de dois provedores medidos{nota}"))
            } else if medidos.iter().any(|p| faixa_de(p, nome).is_none()) {
                (None, format!("não medida em todos{nota}"))
            } else {
                let cands: Vec<(&str, Faixa)> = medidos
                    .iter()
                    .filter_map(|p| Some((p.provedor.as_str(), faixa_de(p, nome)?)))
                    .collect();
                let v = vencedor_entre(&cands, *maior);
                let m = if v.is_some() {
                    "intervalo de 95% separado de todos"
                } else {
                    "intervalos de 95% se cruzam"
                };
                (v, format!("{m}{nota}"))
            };
            Vencedor {
                metrica: nome.to_string(),
                modelo,
                motivo,
            }
        })
        .collect()
}

// ------------------------------------------------------------------ o arena, como registro

fn registro_do_arena(
    provedores: &[String],
    b: &Bateria,
    ensaios: &[Ensaio],
) -> Result<Vec<JanelaDoArena>, String> {
    if provedores.len() < 2 {
        return Ok(vec![]);
    }
    let sha = |v: &serde_json::Value| sha256_hex(v.to_string().as_bytes());
    let chave = |p: &str| arena::CandidateKey {
        // O provedor e o nome: uuid nulo para todos, e o `model_id` os distingue.
        provider_uuid: uuid::Uuid::nil(),
        model_id: p.to_string(),
    };
    let agora = chrono::Utc::now().timestamp();
    let spec = arena::ArenaSpec {
        tenant_uuid: uuid::Uuid::nil(),
        arena_uuid: uuid::Uuid::now_v7(),
        name: "phxclaw avaliar --bateria".into(),
        mode: arena::ArenaMode::Paired,
        task_family: phxclaw_ai_benchmark::TaskFamily::General,
        complexity: phxclaw_ai_benchmark::ComplexityBand::Low,
        evidence_class: phxclaw_ai_benchmark::EvidenceClass::Fixture,
        champion: chave(&provedores[0]),
        challengers: provedores[1..].iter().map(|p| chave(p)).collect(),
        challenger_traffic_basis_points: 10_000,
        assignment_salt_sha256: sha(&json!(["bateria", b.semente])),
        policy_sha256: sha(&json!(
            "vencedor: faixas de 95% por bootstrap que nao se cruzam"
        )),
        min_paired_samples: 1,
        required_consecutive_wins: 1,
        min_quality_improvement_basis_points: 0,
        min_success_improvement_basis_points: 0,
        max_quality_regression_basis_points: 0,
        max_success_regression_basis_points: 0,
        max_latency_regression_basis_points: 0,
        max_cost_regression_basis_points: 0,
        starts_at_unix: agora - 1,
        expires_at_unix: agora + 86_400,
    };
    arena::validate_spec(&spec).map_err(|e| format!("arena: {e}"))?;
    let doc = arena::VerifiedArenaDocument {
        document_sha256: sha256_hex(&arena::canonical_spec_bytes(&spec)),
        // Sem assinatura de proposito: documento de bateria local, evidencia Fixture.
        signer_id: "phxclaw-bateria-local".into(),
        spec,
    };
    let dataset = sha(&json!(b.casos));
    let scorer = sha(&json!("gabarito: contem (sem caixa) + sequencia exata; v2"));
    let ambiente = sha(&json!([std::env::consts::OS, std::env::consts::ARCH]));
    let uuids: BTreeMap<&str, uuid::Uuid> = b
        .casos
        .iter()
        .map(|c| (c.id.as_str(), uuid::Uuid::now_v7()))
        .collect();
    let resultado = |e: &Ensaio| arena::MetricOutcome {
        success: e.acerto,
        quality_basis_points: if e.acerto { 10_000 } else { 0 },
        latency_ms: e.duracao_ms.round() as u64,
        actual_cost_micro_usd: e.custo_micro_usd,
        safety_violation: false,
        output_sha256: e.resposta_sha256.clone(),
    };
    let mut janelas = Vec::new();
    for d in &provedores[1..] {
        let mut obs = Vec::new();
        for c in ensaios.iter().filter(|e| e.provedor == provedores[0]) {
            let Some(x) = ensaios
                .iter()
                .find(|e| &e.provedor == d && e.caso == c.caso && e.rodada == c.rodada)
            else {
                continue;
            };
            obs.push(arena::ArenaPairObservation {
                tenant_uuid: doc.spec.tenant_uuid,
                observation_uuid: uuid::Uuid::now_v7(),
                arena_uuid: doc.spec.arena_uuid,
                request_uuid: uuid::Uuid::now_v7(),
                case_uuid: uuids[c.caso.as_str()],
                evidence_class: doc.spec.evidence_class,
                champion: doc.spec.champion.clone(),
                challenger: chave(d),
                champion_outcome: resultado(c),
                challenger_outcome: resultado(x),
                dataset_sha256: dataset.clone(),
                scorer_sha256: scorer.clone(),
                environment_sha256: ambiente.clone(),
                observed_at_unix: agora,
            });
        }
        let janela =
            arena::aggregate_window(&doc, &chave(d), &obs).map_err(|e| format!("arena: {e}"))?;
        janelas.push(JanelaDoArena {
            campeao: provedores[0].clone(),
            desafiante: d.clone(),
            veredito_do_arena: arena::evaluate_window(&doc, &janela),
            janela,
        });
    }
    Ok(janelas)
}

/// A tabela do terminal: cada metrica com o intervalo de 95% e o N de casos.
pub fn tabela(r: &ResultadoBateria) -> String {
    let f = |m: &Medida, casas: usize| match m {
        Medida::Faixa(x) => format!(
            "{:.c$} [{:.c$}–{:.c$}] N={} casos",
            x.mediana,
            x.min,
            x.max,
            x.n,
            c = casas
        ),
        Medida::NaoMedida(motivo) => format!("não medida ({motivo})"),
    };
    let mut s = format!(
        "bateria de {} — {} caso(s) x {} rodada(s), até {} tentativa(s); IC 95% por bootstrap ({} reamostras, semente {})\n",
        r.data, r.casos, r.rodadas, r.tentativas, r.reamostras, r.semente
    );
    for p in &r.provedores {
        s.push_str(&format!("\n{}  (N={} ensaios)\n", p.provedor, p.ensaios));
        if let Some(m) = &p.nao_medido {
            s.push_str(&format!("  {m}\n"));
        }
        s.push_str(&format!(
            "  acerto       : {}\n  custo/acerto : {}\n  duração ms   : {}\n  tentativas   : {}\n  intervenção  : {}\n",
            f(&p.acerto, 2),
            f(&p.custo_por_acerto, 6),
            f(&p.duracao_ms, 0),
            f(&p.tentativas, 2),
            f(&p.intervencao, 2),
        ));
    }
    s.push_str("\nvencedor (só quando os intervalos de 95% não se cruzam):\n");
    for v in &r.vencedores {
        s.push_str(&format!(
            "  {:<17} {}\n",
            v.metrica,
            match &v.modelo {
                Some(m) => format!("{m} ({})", v.motivo),
                None => format!("sem vencedor ({})", v.motivo),
            }
        ));
    }
    if !r.arena.is_empty() {
        s.push_str("\nregistro pareado do phxclaw-model-arena (média; informativo, não decide):\n");
        for j in &r.arena {
            s.push_str(&format!(
                "  {} x {}: {} pares, delta de acerto {} bp, veredito {:?}, janela {}\n",
                j.campeao,
                j.desafiante,
                j.janela.sample_count,
                j.janela.success_delta_basis_points,
                j.veredito_do_arena,
                &j.janela.window_sha256[..12]
            ));
        }
    }
    if let Some(e) = &r.arena_erro {
        s.push_str(&format!("\narena: {e}\n"));
    }
    s
}

#[cfg(test)]
mod testes {
    use super::*;

    fn caso(acerto: f64, custo: Option<f64>) -> PorCaso {
        PorCaso {
            acerto,
            acertos: acerto,
            custo,
            duracao: 100.0,
            tentativas: 1.0,
            intervencao: 0.0,
        }
    }

    #[test]
    fn sorteio_e_deterministico_e_ponta_por_posto() {
        let mut a = Sorteio(7);
        let mut b = Sorteio(7);
        let va: Vec<usize> = (0..50).map(|_| a.ate(10)).collect();
        let vb: Vec<usize> = (0..50).map(|_| b.ate(10)).collect();
        assert_eq!(va, vb);
        assert!(va.iter().all(|x| *x < 10) && va.iter().any(|x| *x != va[0]));
        let v: Vec<f64> = (1..=1000).map(f64::from).collect();
        assert_eq!(ponta(&v, 25), 25.0);
        assert_eq!(ponta(&v, 975), 975.0);
    }

    #[test]
    fn custo_por_acerto_sem_acerto_ou_sem_preco_nao_e_numero() {
        let idx = vec![vec![0, 1]; 300];
        let sem_preco = [caso(1.0, Some(1.0)), caso(1.0, None)];
        assert!(
            matches!(&intervalos(&sem_preco, &idx)[1], Medida::NaoMedida(m) if m.contains("preço"))
        );
        let sem_acerto = [caso(0.0, Some(1.0)), caso(0.0, Some(1.0))];
        assert!(
            matches!(&intervalos(&sem_acerto, &idx)[1], Medida::NaoMedida(m) if m.contains("sem denominador"))
        );
    }

    #[test]
    fn bateria_errada_e_recusada() {
        let c = |id: &str| Caso {
            id: id.into(),
            objetivo: "x".into(),
            gabarito: crate::avaliacao::Gabarito {
                contem: vec!["y".into()],
                sequencia: None,
            },
        };
        let b = |casos, rodadas, reamostras| Bateria {
            casos,
            rodadas,
            tentativas: 1,
            reamostras,
            semente: 1,
        };
        assert!(
            conferir(&b(vec![], 3, 1000))
                .unwrap_err()
                .contains("sem casos")
        );
        assert!(
            conferir(&b(vec![c("a"), c("a")], 3, 1000))
                .unwrap_err()
                .contains("repetido")
        );
        assert!(
            conferir(&b(vec![c("a")], 0, 1000))
                .unwrap_err()
                .contains("rodadas")
        );
        assert!(
            conferir(&b(vec![c("a")], 3, 10))
                .unwrap_err()
                .contains("reamostras")
        );
        assert!(conferir(&b(vec![c("a")], 3, 1000)).is_ok());
    }
}
