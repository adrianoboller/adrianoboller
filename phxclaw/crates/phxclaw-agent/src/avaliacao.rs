//! Avaliacao de modelos pelo caminho que o agente usa de verdade: cada execucao e uma
//! tarefa inteira no motor (montagem, portao, ferramentas), nao uma chamada solta ao
//! modelo. Mede latencia (p50/p95), tokens/s, segundos de CPU, energia e acerto por
//! gabarito, cada numero com faixa min-max, N e data.
//!
//! Duas regras que esta casa ja pagou, e por isso estao no codigo e nao so no documento:
//! - **vencedor so quando as faixas nao se cruzam** (`faixas_decidem`): mediana melhor
//!   dentro do ruido nao e vitoria. A mesma funcao decide a promocao de skill.
//! - **numero so medido**: tokens/s sai do `eval_duration` que o provedor informa (o
//!   Ollama), nunca do relogio de parede; energia sai do RAPL ou do contador da NVIDIA, e
//!   sem eles o campo diz «não medida (sem RAPL)» -- nunca uma estimativa.
//!
//! Alem do acerto (tudo ou nada), a **nota parcial de ferramentas** (SP000030, a
//! `ToolCorrectness` do DeepEval, deterministica): `conjunto` e quantas das esperadas foram
//! chamadas, `sequencia` e a maior subsequencia comum na ordem, as duas de 0 a 1 sobre o
//! gabarito. Nunca juiz por modelo: nota que sai de outro modelo e opiniao com casa
//! decimal, e a faixa dela mediria o juiz, nao o avaliado.
//!
//! As execucoes saem agrupadas pelo `prompt_sha256` e pelo sha das skills que a gravacao
//! registrou no primeiro pedido: antes/depois de mexer no prompt se compara o MESMO prompt,
//! e nao duas corridas que mudaram duas coisas.
//!
//! A observacao e o perfil (acerto ponderado, p95) sao os do `phxclaw-ai-benchmark`; esta
//! camada acrescenta o que ele nao guarda (p50, faixas, CPU, energia). O
//! `phxclaw-model-arena` NAO entra: ele decide por diferenca de MEDIA entre campeao e
//! desafiante, com documento assinado e perfil promovido -- outra pergunta, e uma que a
//! regra das faixas recusaria.

use crate::gravacao::{Gravacao, Gravador, gravando};
use crate::motor::{Agent, CancelFlag, NoObserver, sha256_hex};
use crate::tarefa::{Task, TaskStatus};
use phxclaw_agent_core::{BoxFut, Llm, LlmError, LlmOptions, LlmReply, Message, ToolSpec};
use phxclaw_ai_benchmark::{
    BenchmarkCase, BenchmarkObservation, BenchmarkSuite, ComplexityBand, EvidenceClass, TaskFamily,
    aggregate_profile,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Instant;

// ------------------------------------------------------------------ faixa e decisao

/// Faixa de uma metrica: N amostras, minimo, mediana e maximo.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Faixa {
    pub n: usize,
    pub min: f64,
    pub mediana: f64,
    pub max: f64,
}

impl Faixa {
    pub fn de(v: &[f64]) -> Option<Faixa> {
        if v.is_empty() {
            return None;
        }
        let mut s = v.to_vec();
        s.sort_by(f64::total_cmp);
        Some(Faixa {
            n: s.len(),
            min: s[0],
            mediana: percentil(&s, 50),
            max: s[s.len() - 1],
        })
    }
}

/// Percentil por posto mais proximo, o mesmo metodo do p95 do ai-benchmark: os dois numeros
/// da mesma tabela nao podem vir de definicoes diferentes. `s` ja ordenado.
pub fn percentil(s: &[f64], p: usize) -> f64 {
    let i = (s.len() * p)
        .div_ceil(100)
        .saturating_sub(1)
        .min(s.len() - 1);
    s[i]
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Lado {
    A,
    B,
}

/// O UNICO juiz de «quem ganhou» desta base de medicao: so ha vencedor quando as faixas
/// min-max nao se tocam. Encostar (max de um igual ao min do outro) e cruzar.
pub fn faixas_decidem(a: &Faixa, b: &Faixa, maior_e_melhor: bool) -> Option<Lado> {
    let (melhor_se_b_acima, melhor_se_a_acima) = if maior_e_melhor {
        (Lado::B, Lado::A)
    } else {
        (Lado::A, Lado::B)
    };
    if a.max < b.min {
        Some(melhor_se_b_acima)
    } else if b.max < a.min {
        Some(melhor_se_a_acima)
    } else {
        None
    }
}

// ------------------------------------------------------------------ energia

/// Energia de uma execucao, ou o motivo de nao ter sido medida.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Energia {
    Medida { joules: f64, fonte: String },
    NaoMedida(String),
}

impl std::fmt::Display for Energia {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Energia::Medida { joules, fonte } => write!(f, "{joules:.3} J ({fonte})"),
            Energia::NaoMedida(m) => write!(f, "não medida ({m})"),
        }
    }
}

/// Zona RAPL de topo: `energy_uj` e o teto onde o contador da a volta.
#[derive(Debug, Clone)]
struct Zona {
    arquivo: PathBuf,
    volta_uj: u64,
}

/// Leitor de energia do hospedeiro. A raiz do `/sys` e parametro para a prova rodar contra
/// uma arvore falsa: aqui nao ha RAPL, e a conta so se prova com um `energy_uj` de mentira.
#[derive(Debug, Clone)]
pub struct LeitorEnergia {
    zonas: Vec<Zona>,
    /// Motivo de nao haver RAPL legivel, para a mensagem dizer qual dos dois faltou.
    sem_rapl: Option<String>,
    nvidia: Option<PathBuf>,
}

#[derive(Debug, Clone, Default)]
pub struct AmostraEnergia {
    rapl_uj: Vec<Option<u64>>,
    nvidia_mj: Option<u64>,
}

impl LeitorEnergia {
    pub fn do_sistema() -> Self {
        Self::de_raiz(Path::new("/sys"), achar_nvidia_smi())
    }

    /// So zonas de topo `intel-rapl:N` (pacote): as subzonas `intel-rapl:N:M` ja estao
    /// dentro do pacote e a `intel-rapl-mmio` repete o mesmo dominio -- somar qualquer
    /// uma delas contaria a mesma energia duas vezes. A `psys` (plataforma inteira)
    /// tambem contem o pacote e fica de fora pelo mesmo motivo.
    pub fn de_raiz(sys: &Path, nvidia: Option<PathBuf>) -> Self {
        let dir = sys.join("class/powercap");
        let mut zonas = Vec::new();
        let mut sem_rapl = Some("sem RAPL".to_string());
        if let Ok(rd) = std::fs::read_dir(&dir) {
            let mut nomes: Vec<_> = rd.flatten().map(|e| e.path()).collect();
            nomes.sort();
            for p in nomes {
                let nome = p.file_name().and_then(|n| n.to_str()).unwrap_or_default();
                let topo = nome
                    .strip_prefix("intel-rapl:")
                    .is_some_and(|n| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()));
                if !topo {
                    continue;
                }
                let dominio = std::fs::read_to_string(p.join("name")).unwrap_or_default();
                if dominio.trim() == "psys" {
                    continue;
                }
                let arquivo = p.join("energy_uj");
                match std::fs::read_to_string(&arquivo) {
                    Ok(t) if t.trim().parse::<u64>().is_ok() => {
                        let volta_uj = std::fs::read_to_string(p.join("max_energy_range_uj"))
                            .ok()
                            .and_then(|t| t.trim().parse().ok())
                            .unwrap_or(0);
                        zonas.push(Zona { arquivo, volta_uj });
                    }
                    // Desde 2020 o `energy_uj` e so do root (canal lateral): existe e nao se
                    // le e diferente de nao existir, e a mensagem diz qual.
                    _ => sem_rapl = Some("RAPL sem permissão de leitura".into()),
                }
            }
        }
        if !zonas.is_empty() {
            sem_rapl = None;
        }
        Self {
            zonas,
            sem_rapl,
            nvidia,
        }
    }

    pub fn amostra(&self) -> AmostraEnergia {
        AmostraEnergia {
            rapl_uj: self
                .zonas
                .iter()
                .map(|z| {
                    std::fs::read_to_string(&z.arquivo)
                        .ok()
                        .and_then(|t| t.trim().parse().ok())
                })
                .collect(),
            nvidia_mj: self.nvidia.as_deref().and_then(ler_nvidia_mj),
        }
    }

    /// A energia entre duas amostras. Sem nenhuma fonte, diz por que, e nunca estima.
    pub fn entre(&self, antes: &AmostraEnergia, depois: &AmostraEnergia) -> Energia {
        let mut uj: u64 = 0;
        let mut fontes = Vec::new();
        if !self.zonas.is_empty() {
            for ((z, a), d) in self.zonas.iter().zip(&antes.rapl_uj).zip(&depois.rapl_uj) {
                let (Some(a), Some(d)) = (a, d) else {
                    return Energia::NaoMedida("RAPL ilegível no meio da medição".into());
                };
                // O contador da a volta em `max_energy_range_uj`; sem o teto conhecido, a
                // volta nao se desfaz, e a medida e recusada em vez de sair negativa.
                uj += if d >= a {
                    d - a
                } else if z.volta_uj > *a {
                    z.volta_uj - a + d
                } else {
                    return Energia::NaoMedida("RAPL deu a volta sem teto conhecido".into());
                };
            }
            fontes.push(format!("RAPL, {} zona(s)", self.zonas.len()));
        }
        let mut joules = uj as f64 / 1e6;
        if let (Some(a), Some(d)) = (antes.nvidia_mj, depois.nvidia_mj) {
            joules += d.saturating_sub(a) as f64 / 1e3;
            fontes.push("NVIDIA".into());
        }
        if fontes.is_empty() {
            let mut m = self.sem_rapl.clone().unwrap_or_else(|| "sem RAPL".into());
            if self.nvidia.is_some() {
                m.push_str("; nvidia-smi sem total_energy_consumption");
            }
            return Energia::NaoMedida(m);
        }
        Energia::Medida {
            joules,
            fonte: fontes.join(" + "),
        }
    }
}

fn achar_nvidia_smi() -> Option<PathBuf> {
    std::env::var_os("PATH").and_then(|p| {
        std::env::split_paths(&p)
            .map(|d| d.join("nvidia-smi"))
            .find(|f| f.is_file())
    })
}

/// Contador acumulado de energia das GPUs (mJ, desde o carregamento do driver). E a unica
/// leitura de energia da NVIDIA: `power.draw` e potencia instantanea, e integra-la por
/// amostragem seria estimar.
fn ler_nvidia_mj(nvidia: &Path) -> Option<u64> {
    let out = std::process::Command::new(nvidia)
        .args([
            "--query-gpu=total_energy_consumption",
            "--format=csv,noheader,nounits",
        ])
        .env_clear()
        .output()
        .ok()
        .filter(|o| o.status.success())?;
    somar_nvidia(&String::from_utf8_lossy(&out.stdout))
}

/// Soma uma linha por GPU. Qualquer GPU sem o contador (`[N/A]`, placa antiga) anula a
/// soma: energia de parte das placas apresentada como o total seria mentira.
pub fn somar_nvidia(texto: &str) -> Option<u64> {
    let mut total = 0u64;
    let mut alguma = false;
    for l in texto.lines().map(str::trim).filter(|l| !l.is_empty()) {
        total += l.parse::<f64>().ok().filter(|v| *v >= 0.0)? as u64;
        alguma = true;
    }
    alguma.then_some(total)
}

// ------------------------------------------------------------------ CPU

/// O `/proc/*/stat` conta em USER_HZ, que o kernel fixa em 100 para o espaco de usuario
/// (a ABI nao muda com o HZ interno); ler o `sysconf` pediria `unsafe`.
const USER_HZ: f64 = 100.0;

/// Segundos de CPU (usuario + sistema) de um processo; `com_filhos` soma os filhos ja
/// esperados (`cutime`/`cstime`), que e onde caem as ferramentas que rodam no bwrap.
pub fn cpu_s(proc_dir: &Path, com_filhos: bool) -> Option<f64> {
    let t = std::fs::read_to_string(proc_dir.join("stat")).ok()?;
    let (_, depois) = t.rsplit_once(')')?;
    let campos: Vec<&str> = depois.split_whitespace().collect();
    // Depois do `)`, o campo 0 e o estado (campo 3 do stat): utime e o 14, logo o 11 aqui.
    let n = |i: usize| campos.get(i).and_then(|c| c.parse::<u64>().ok());
    let mut ticks = n(11)? + n(12)?;
    if com_filhos {
        ticks += n(13)? + n(14)?;
    }
    Some(ticks as f64 / USER_HZ)
}

/// CPU por pid de todo processo cujo `comm` e `nome` E de todos os descendentes dele. O
/// servidor do modelo nao gera sozinho: o Ollama sobe um filho por modelo carregado, e o
/// nome do filho muda de versao para versao (`ollama_llama_server`, `llama-server`) --
/// medido aqui: o `ollama serve` tinha 1,2 s de CPU e o `llama-server` filho, 353 s.
/// Contar pelo parentesco nao depende do nome que a versao deu ao filho.
pub fn cpu_da_arvore(proc_raiz: &Path, nome: &str) -> BTreeMap<u32, f64> {
    let mut todos: Vec<(u32, u32, String, PathBuf)> = Vec::new();
    if let Ok(rd) = std::fs::read_dir(proc_raiz) {
        for e in rd.flatten() {
            let Some(pid) = e.file_name().to_str().and_then(|s| s.parse::<u32>().ok()) else {
                continue;
            };
            let Ok(stat) = std::fs::read_to_string(e.path().join("stat")) else {
                continue;
            };
            let ppid = stat
                .rsplit_once(')')
                .and_then(|(_, d)| d.split_whitespace().nth(1)?.parse().ok())
                .unwrap_or(0);
            let comm = std::fs::read_to_string(e.path().join("comm")).unwrap_or_default();
            todos.push((pid, ppid, comm.trim().to_string(), e.path()));
        }
    }
    let mut dentro: std::collections::BTreeSet<u32> = todos
        .iter()
        .filter(|(_, _, c, _)| c == nome)
        .map(|(p, ..)| *p)
        .collect();
    // Desce a arvore ate parar de crescer: a ordem do /proc nao garante pai antes de filho.
    loop {
        let antes = dentro.len();
        for (pid, ppid, ..) in &todos {
            if dentro.contains(ppid) {
                dentro.insert(*pid);
            }
        }
        if dentro.len() == antes {
            break;
        }
    }
    todos
        .iter()
        .filter(|(p, ..)| dentro.contains(p))
        .filter_map(|(p, _, _, dir)| cpu_s(dir, false).map(|s| (*p, s)))
        .collect()
}

/// CPU gasta entre duas leituras. Processo que nasceu no meio entra inteiro (nasceu para
/// esta execucao); o que morreu no meio sai da conta, e isso fica escrito no resultado.
pub fn delta_cpu(antes: &BTreeMap<u32, f64>, depois: &BTreeMap<u32, f64>) -> f64 {
    depois
        .iter()
        .map(|(pid, d)| d - antes.get(pid).copied().unwrap_or(0.0))
        .filter(|x| *x >= 0.0)
        .sum()
}

// ------------------------------------------------------------------ medidor do modelo

#[derive(Debug, Clone, Default)]
struct Geracao {
    tokens: u64,
    ns: Option<u64>,
}

/// Envolve o modelo e guarda o que cada resposta diz da propria geracao.
struct MedidorLlm {
    interno: Arc<dyn Llm>,
    visto: Arc<Mutex<Vec<Geracao>>>,
}

impl Llm for MedidorLlm {
    fn id(&self) -> String {
        self.interno.id()
    }
    fn chat<'a>(
        &'a self,
        messages: &'a [Message],
        tools: &'a [ToolSpec],
        options: &'a LlmOptions,
    ) -> BoxFut<'a, Result<LlmReply, LlmError>> {
        Box::pin(async move {
            let r = self.interno.chat(messages, tools, options).await;
            if let Ok(resp) = &r {
                self.visto.lock().unwrap().push(Geracao {
                    tokens: resp.usage.output_tokens,
                    ns: resp.usage.duracao_geracao_ns,
                });
            }
            r
        })
    }
}

/// tokens/s de uma execucao: soma dos tokens sobre a soma do tempo de geracao. So existe
/// se TODA resposta informou o tempo -- somar so as que informaram inflaria o numero.
fn tokens_por_s(g: &[Geracao]) -> Option<f64> {
    if g.is_empty() || g.iter().any(|x| x.ns.is_none()) {
        return None;
    }
    let ns: u64 = g.iter().filter_map(|x| x.ns).sum();
    let tok: u64 = g.iter().map(|x| x.tokens).sum();
    (ns > 0).then(|| tok as f64 / (ns as f64 / 1e9))
}

// ------------------------------------------------------------------ casos e gabarito

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Gabarito {
    /// Cada texto tem de aparecer na resposta final (sem diferenciar maiusculas).
    #[serde(default)]
    pub contem: Vec<String>,
    /// A sequencia exata de ferramentas que o motor chamou (a de uma gravacao).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sequencia: Option<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Caso {
    pub id: String,
    pub objetivo: String,
    #[serde(default)]
    pub gabarito: Gabarito,
}

/// `*.json` e caso com gabarito escrito; `*.jsonl` e gravacao, e o gabarito e a sequencia
/// de ferramentas que ela registrou -- a corrida boa gravada vira a regua das outras.
pub fn ler_casos(dir: &Path) -> Result<Vec<Caso>, String> {
    let mut arqs: Vec<PathBuf> = std::fs::read_dir(dir)
        .map_err(|e| format!("{}: {e}", dir.display()))?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "json" || x == "jsonl"))
        .collect();
    arqs.sort();
    let mut casos = Vec::new();
    for p in arqs {
        let id = p
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or_default()
            .to_string();
        let caso = if p.extension().is_some_and(|x| x == "jsonl") {
            let g = Gravacao::ler(&p)?;
            Caso {
                id,
                objetivo: g.objetivo.clone(),
                gabarito: Gabarito {
                    contem: vec![],
                    sequencia: Some(g.sequencia()),
                },
            }
        } else {
            let t = std::fs::read_to_string(&p).map_err(|e| format!("{}: {e}", p.display()))?;
            let mut c: Caso =
                serde_json::from_str(&t).map_err(|e| format!("{}: {e}", p.display()))?;
            if c.id.is_empty() {
                c.id = id;
            }
            c
        };
        if caso.gabarito.contem.is_empty() && caso.gabarito.sequencia.is_none() {
            return Err(format!(
                "{}: caso sem gabarito (contem ou sequencia)",
                p.display()
            ));
        }
        casos.push(caso);
    }
    if casos.is_empty() {
        return Err(format!(
            "{}: nenhum caso (*.json ou *.jsonl)",
            dir.display()
        ));
    }
    Ok(casos)
}

pub fn acertou(g: &Gabarito, t: &Task, sequencia: &[String]) -> bool {
    if t.status != TaskStatus::Completed {
        return false;
    }
    let resposta = t.answer.clone().unwrap_or_default().to_lowercase();
    g.contem
        .iter()
        .all(|c| resposta.contains(&c.to_lowercase()))
        && g.sequencia.as_deref().is_none_or(|s| s == sequencia)
}

/// A nota parcial de ferramentas, de 0 a 1, as duas sobre o tamanho do gabarito.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct NotaFerramentas {
    /// |esperadas ∩ chamadas| / |esperadas|, como conjuntos (repeticao nao conta duas vezes).
    pub conjunto: f64,
    /// LCS(esperadas, chamadas) / |esperadas|: o quanto da ORDEM esperada foi seguida.
    pub sequencia: f64,
}

/// Comprimento da maior subsequencia comum, O(|a|·|b|) -- sequencias de ferramenta tem
/// dezenas de itens, nao milhares.
pub fn lcs(a: &[String], b: &[String]) -> usize {
    let mut prev = vec![0usize; b.len() + 1];
    let mut cur = vec![0usize; b.len() + 1];
    for x in a {
        for (j, y) in b.iter().enumerate() {
            cur[j + 1] = if x == y {
                prev[j] + 1
            } else {
                prev[j + 1].max(cur[j])
            };
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev[b.len()]
}

/// `None` sem gabarito de sequencia: nota sobre zero esperadas nao e nota. Chamada a mais
/// nao desconta de proposito -- e o mesmo denominador do DeepEval, e descontar misturaria
/// «fez o que devia» com «fez so o que devia», que o acerto exato ja cobra.
pub fn nota_ferramentas(esperadas: &[String], chamadas: &[String]) -> Option<NotaFerramentas> {
    if esperadas.is_empty() {
        return None;
    }
    let e: std::collections::BTreeSet<&String> = esperadas.iter().collect();
    let c: std::collections::BTreeSet<&String> = chamadas.iter().collect();
    Some(NotaFerramentas {
        conjunto: e.intersection(&c).count() as f64 / e.len() as f64,
        sequencia: lcs(esperadas, chamadas) as f64 / esperadas.len() as f64,
    })
}

// ------------------------------------------------------------------ avaliacao

/// Uma execucao (modelo x caso x rodada), com tudo que se mediu nela.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Execucao {
    pub modelo: String,
    pub caso: String,
    pub rodada: usize,
    pub acerto: bool,
    /// A nota parcial de ferramentas; ausente quando o caso nao tem gabarito de sequencia.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nota_ferramentas: Option<NotaFerramentas>,
    /// O que a gravacao registrou no primeiro pedido: o prompt e as skills sob os quais
    /// esta execucao correu.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skills_sha256: Option<String>,
    pub estado: String,
    /// O erro da tarefa que nao concluiu (modelo que recusa ferramenta, prazo...).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub erro: Option<String>,
    pub latencia_ms: f64,
    pub tokens_saida: u64,
    pub tokens_por_s: Option<f64>,
    pub cpu_s_agente: Option<f64>,
    pub cpu_s_modelo: Option<f64>,
    pub energia: Energia,
    pub gravacao: PathBuf,
}

/// Metrica com faixa, ou o motivo de nao ter sido medida.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Medida {
    Faixa(Faixa),
    NaoMedida(String),
}

impl Medida {
    fn de(v: &[Option<f64>], motivo: &str) -> Medida {
        // Faixa com parte das execucoes medida seria um numero de outra populacao.
        match v.iter().copied().collect::<Option<Vec<f64>>>() {
            Some(x) if !x.is_empty() => Medida::Faixa(Faixa::de(&x).unwrap()),
            _ => Medida::NaoMedida(motivo.into()),
        }
    }
    pub fn faixa(&self) -> Option<&Faixa> {
        match self {
            Medida::Faixa(f) => Some(f),
            Medida::NaoMedida(_) => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResultadoModelo {
    pub modelo: String,
    pub execucoes: usize,
    /// Execucoes que nao concluiram. Modelo com falha fica FORA do vencedor de desempenho:
    /// a latencia de quem morre no primeiro pedido nao e rapidez.
    pub falhas: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub primeira_falha: Option<String>,
    pub latencia_ms: Faixa,
    pub p50_ms: f64,
    /// Do perfil do ai-benchmark (posto mais proximo), nao recalculado aqui.
    pub p95_ms: u64,
    pub tokens_por_s: Medida,
    pub cpu_s_agente: Medida,
    pub cpu_s_modelo: Medida,
    pub energia_j: Medida,
    /// Taxa de acerto POR RODADA (0..1): a faixa e entre rodadas, cada uma com todos os
    /// casos -- e a unidade que se repete.
    pub acerto_por_rodada: Faixa,
    /// Mediana, por rodada, da nota de conjunto dos casos; a faixa e entre rodadas. Nao
    /// medida quando algum caso nao tem gabarito de sequencia.
    #[serde(default = "nao_medida_sem_sequencia")]
    pub nota_conjunto_por_rodada: Medida,
    #[serde(default = "nao_medida_sem_sequencia")]
    pub nota_sequencia_por_rodada: Medida,
    /// Acerto ponderado de todas as execucoes, em pontos-base, do perfil do ai-benchmark.
    pub acerto_bp: u16,
    pub perfil_sha256: String,
}

fn nao_medida_sem_sequencia() -> Medida {
    Medida::NaoMedida("caso sem gabarito de sequencia".into())
}

/// As execucoes de um modelo sob o MESMO prompt e as mesmas skills: a unidade que se
/// compara antes/depois de mexer num deles.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResultadoPorPrompt {
    pub modelo: String,
    pub prompt_sha256: Option<String>,
    pub skills_sha256: Option<String>,
    pub execucoes: usize,
    pub acertos: usize,
    /// Faixa das notas de sequencia das execucoes do grupo.
    pub nota_sequencia: Medida,
}

/// Agrupa por (modelo, prompt, skills), na ordem em que aparecem. Vale sobre um
/// `resultado.json` ja gravado tanto quanto sobre a avaliacao que acabou de rodar.
pub fn agrupar_por_prompt(ex: &[Execucao]) -> Vec<ResultadoPorPrompt> {
    let mut grupos: Vec<ResultadoPorPrompt> = Vec::new();
    let mut notas: Vec<Vec<Option<f64>>> = Vec::new();
    for e in ex {
        let chave = (&e.modelo, &e.prompt_sha256, &e.skills_sha256);
        let i = match grupos
            .iter()
            .position(|g| (&g.modelo, &g.prompt_sha256, &g.skills_sha256) == chave)
        {
            Some(i) => i,
            None => {
                grupos.push(ResultadoPorPrompt {
                    modelo: e.modelo.clone(),
                    prompt_sha256: e.prompt_sha256.clone(),
                    skills_sha256: e.skills_sha256.clone(),
                    execucoes: 0,
                    acertos: 0,
                    nota_sequencia: nao_medida_sem_sequencia(),
                });
                notas.push(vec![]);
                grupos.len() - 1
            }
        };
        grupos[i].execucoes += 1;
        grupos[i].acertos += usize::from(e.acerto);
        notas[i].push(e.nota_ferramentas.map(|n| n.sequencia));
    }
    for (g, n) in grupos.iter_mut().zip(&notas) {
        g.nota_sequencia = Medida::de(n, "caso sem gabarito de sequencia");
    }
    grupos
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Vencedor {
    pub metrica: String,
    /// `None` quando as faixas se cruzam: empate, dito como empate.
    pub modelo: Option<String>,
    pub motivo: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Avaliacao {
    pub data: String,
    pub rodadas: usize,
    pub casos: usize,
    pub modelos: Vec<ResultadoModelo>,
    pub vencedores: Vec<Vencedor>,
    #[serde(default)]
    pub por_prompt: Vec<ResultadoPorPrompt>,
    pub execucoes: Vec<Execucao>,
}

/// Monta o agente de um modelo (a mesma forma da `AgentFactory` da API).
pub type Fabrica<'a> = &'a (dyn Fn(&str) -> Result<Agent, String> + Sync);

/// Avalia `modelos` sobre `casos`, `rodadas` vezes, uma execucao por vez (execucoes em
/// paralelo disputariam a CPU e o modelo, e a latencia mediria a fila). Cada execucao e
/// gravada em `saida/gravacoes/`, repetivel pelo `phxclaw repetir`.
pub async fn avaliar(
    fabrica: Fabrica<'_>,
    modelos: &[String],
    casos: &[Caso],
    rodadas: usize,
    saida: &Path,
    energia: &LeitorEnergia,
) -> Result<Avaliacao, String> {
    if modelos.is_empty() || casos.is_empty() || rodadas == 0 {
        return Err("avaliar pede ao menos um modelo, um caso e uma rodada".into());
    }
    let dir_grav = saida.join("gravacoes");
    std::fs::create_dir_all(&dir_grav).map_err(|e| format!("{}: {e}", dir_grav.display()))?;
    let suite = suite_de(casos);
    let mut resultados = Vec::new();
    let mut execucoes = Vec::new();
    for modelo in modelos {
        let provider = uuid::Uuid::now_v7();
        let mut obs = Vec::new();
        let mut minhas = Vec::new();
        for rodada in 0..rodadas {
            let run_uuid = uuid::Uuid::now_v7();
            for (caso, bc) in casos.iter().zip(&suite.cases) {
                let base = fabrica(modelo)?;
                let visto = Arc::new(Mutex::new(Vec::new()));
                let mut agente = base;
                agente.llm = Arc::new(MedidorLlm {
                    interno: agente.llm,
                    visto: visto.clone(),
                });
                let arq = dir_grav.join(format!(
                    "{}-{}-r{}.jsonl",
                    nome_de_arquivo(modelo),
                    nome_de_arquivo(&caso.id),
                    rodada + 1
                ));
                let g = Gravador::criar(&arq, &caso.objetivo, modelo)
                    .map_err(|e| format!("{}: {e}", arq.display()))?;
                let tarefa = Task::new(caso.objetivo.clone(), modelo.clone());
                g.tarefa(&tarefa.id);
                let agente = gravando(agente, &g);
                let ollama = modelo.starts_with("ollama:");
                let proc_raiz = Path::new("/proc");
                let (cpu_a0, cpu_m0) = (
                    cpu_s(&proc_raiz.join("self"), true),
                    cpu_da_arvore(proc_raiz, "ollama"),
                );
                let e0 = energia.amostra();
                let t0 = Instant::now();
                let t = agente
                    .run(tarefa, &CancelFlag::default(), &NoObserver)
                    .await;
                let ms = t0.elapsed().as_secs_f64() * 1e3;
                let e1 = energia.amostra();
                let cpu_a1 = cpu_s(&proc_raiz.join("self"), true);
                let cpu_m1 = cpu_da_arvore(proc_raiz, "ollama");
                g.resultado()?;
                let gravada = Gravacao::ler(&arq)?;
                let seq = gravada.sequencia();
                let ok = acertou(&caso.gabarito, &t, &seq);
                let nota = caso
                    .gabarito
                    .sequencia
                    .as_deref()
                    .and_then(|e| nota_ferramentas(e, &seq));
                let geracoes = visto.lock().unwrap().clone();
                let ex = Execucao {
                    modelo: modelo.clone(),
                    caso: caso.id.clone(),
                    rodada: rodada + 1,
                    acerto: ok,
                    nota_ferramentas: nota,
                    prompt_sha256: gravada.prompt_sha256.clone(),
                    skills_sha256: gravada.skills_sha256_junto(),
                    estado: format!("{:?}", t.status),
                    erro: (t.status != TaskStatus::Completed)
                        .then(|| t.error.clone().unwrap_or_default()),
                    latencia_ms: ms,
                    tokens_saida: geracoes.iter().map(|x| x.tokens).sum(),
                    tokens_por_s: tokens_por_s(&geracoes),
                    cpu_s_agente: cpu_a0.zip(cpu_a1).map(|(a, b)| b - a),
                    cpu_s_modelo: (ollama && !cpu_m1.is_empty())
                        .then(|| delta_cpu(&cpu_m0, &cpu_m1)),
                    energia: energia.entre(&e0, &e1),
                    gravacao: arq,
                };
                obs.push(BenchmarkObservation {
                    tenant_uuid: suite.tenant_uuid,
                    run_uuid,
                    case_uuid: bc.case_uuid,
                    provider_uuid: provider,
                    model_id: modelo.clone(),
                    suite_uuid: suite.suite_uuid,
                    dataset_sha256: suite.dataset_sha256.clone(),
                    scorer_sha256: suite.scorer_sha256.clone(),
                    environment_sha256: suite.environment_sha256.clone(),
                    success: ok,
                    quality_basis_points: if ok { 10_000 } else { 0 },
                    tool_accuracy_basis_points: nota
                        .map(|n| (n.sequencia * 10_000.0).round() as u16),
                    structured_validity_basis_points: None,
                    latency_ms: ms.round() as u64,
                    input_tokens: t.usage.input_tokens,
                    output_tokens: t.usage.output_tokens,
                    actual_cost_micro_usd: None,
                    output_sha256: sha256_hex(t.answer.clone().unwrap_or_default().as_bytes()),
                    error_class: (!ok).then(|| format!("{:?}", t.status)),
                    observed_at_unix: chrono::Utc::now().timestamp(),
                });
                minhas.push(ex);
            }
        }
        let perfil = aggregate_profile(
            &suite,
            provider,
            modelo,
            &obs,
            chrono::Utc::now().timestamp(),
            86_400,
        )
        .map_err(|e| format!("perfil do ai-benchmark: {e}"))?;
        resultados.push(resumir(modelo, &minhas, rodadas, &perfil));
        execucoes.extend(minhas);
    }
    let vencedores = decidir(&resultados);
    Ok(Avaliacao {
        data: chrono::Utc::now().to_rfc3339(),
        rodadas,
        casos: casos.len(),
        modelos: resultados,
        vencedores,
        por_prompt: agrupar_por_prompt(&execucoes),
        execucoes,
    })
}

fn nome_de_arquivo(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '.' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

fn suite_de(casos: &[Caso]) -> BenchmarkSuite {
    let sha = |v: &Value| sha256_hex(v.to_string().as_bytes());
    BenchmarkSuite {
        tenant_uuid: uuid::Uuid::nil(),
        suite_uuid: uuid::Uuid::now_v7(),
        name: "phxclaw avaliar".into(),
        version: "1".into(),
        task_family: TaskFamily::General,
        complexity: ComplexityBand::Low,
        // Casos escritos a mao, nao trafego de producao: o ai-benchmark recusa promover
        // perfil de fixture, e e o certo -- avaliar mede, nao troca o modelo de ninguem.
        evidence_class: EvidenceClass::Fixture,
        dataset_sha256: sha(&json!(casos)),
        scorer_sha256: sha(&json!(
            "gabarito: contem (sem caixa) + sequencia exata; nota de ferramentas conjunto/LCS; v2"
        )),
        environment_sha256: sha(&json!([std::env::consts::OS, std::env::consts::ARCH])),
        cases: casos
            .iter()
            .map(|c| BenchmarkCase {
                case_uuid: uuid::Uuid::now_v7(),
                prompt_sha256: sha(&json!(c.objetivo)),
                expected_contract_sha256: sha(&json!(c.gabarito)),
                tags: Default::default(),
                weight: 1,
            })
            .collect(),
    }
}

fn resumir(
    modelo: &str,
    ex: &[Execucao],
    rodadas: usize,
    perfil: &phxclaw_ai_benchmark::PerformanceProfile,
) -> ResultadoModelo {
    let lat: Vec<f64> = ex.iter().map(|e| e.latencia_ms).collect();
    let mut ord = lat.clone();
    ord.sort_by(f64::total_cmp);
    let por_rodada: Vec<f64> = (1..=rodadas)
        .map(|r| {
            let da: Vec<_> = ex.iter().filter(|e| e.rodada == r).collect();
            da.iter().filter(|e| e.acerto).count() as f64 / da.len().max(1) as f64
        })
        .collect();
    // Mediana dos casos dentro da rodada; `None` na rodada em que algum caso nao tem nota,
    // para a faixa nao misturar rodadas de populacoes diferentes.
    let nota_por_rodada = |pega: fn(&NotaFerramentas) -> f64| -> Vec<Option<f64>> {
        (1..=rodadas)
            .map(|r| {
                let da: Option<Vec<f64>> = ex
                    .iter()
                    .filter(|e| e.rodada == r)
                    .map(|e| e.nota_ferramentas.as_ref().map(pega))
                    .collect();
                da.and_then(|v| Faixa::de(&v)).map(|f| f.mediana)
            })
            .collect()
    };
    let energia: Vec<Option<f64>> = ex
        .iter()
        .map(|e| match &e.energia {
            Energia::Medida { joules, .. } => Some(*joules),
            Energia::NaoMedida(_) => None,
        })
        .collect();
    let motivo_energia = ex
        .iter()
        .find_map(|e| match &e.energia {
            Energia::NaoMedida(m) => Some(m.clone()),
            _ => None,
        })
        .unwrap_or_else(|| "sem RAPL".into());
    let falhas = ex.iter().filter(|e| e.erro.is_some()).count();
    // Sem nenhuma resposta, o motivo de nao haver tokens/s e a falha, nao o provedor.
    let sem_resposta = |motivo: &str| {
        if falhas == ex.len() {
            "todas as execuções falharam".to_string()
        } else {
            motivo.to_string()
        }
    };
    ResultadoModelo {
        modelo: modelo.into(),
        execucoes: ex.len(),
        falhas,
        primeira_falha: ex.iter().find_map(|e| e.erro.clone()),
        latencia_ms: Faixa::de(&lat).unwrap(),
        p50_ms: percentil(&ord, 50),
        p95_ms: perfil.p95_latency_ms,
        tokens_por_s: Medida::de(
            &ex.iter().map(|e| e.tokens_por_s).collect::<Vec<_>>(),
            &sem_resposta("o provedor não informa o tempo de geração"),
        ),
        cpu_s_agente: Medida::de(
            &ex.iter().map(|e| e.cpu_s_agente).collect::<Vec<_>>(),
            "sem /proc/self/stat",
        ),
        cpu_s_modelo: Medida::de(
            &ex.iter().map(|e| e.cpu_s_modelo).collect::<Vec<_>>(),
            &sem_resposta("modelo fora de um processo ollama local"),
        ),
        energia_j: Medida::de(&energia, &motivo_energia),
        acerto_por_rodada: Faixa::de(&por_rodada).unwrap(),
        nota_conjunto_por_rodada: Medida::de(
            &nota_por_rodada(|n| n.conjunto),
            "caso sem gabarito de sequencia",
        ),
        nota_sequencia_por_rodada: Medida::de(
            &nota_por_rodada(|n| n.sequencia),
            "caso sem gabarito de sequencia",
        ),
        acerto_bp: perfil.success_basis_points,
        perfil_sha256: perfil.profile_sha256.clone(),
    }
}

/// Para cada metrica, vence o modelo cuja faixa fica separada da de TODOS os outros, do
/// lado bom. Senao, empate escrito, com o motivo. No acerto todos entram (falha e erro); nas
/// de desempenho, so quem concluiu tudo.
fn decidir(r: &[ResultadoModelo]) -> Vec<Vencedor> {
    type Pega = fn(&ResultadoModelo) -> Option<Faixa>;
    let metricas: [(&str, bool, Pega); 5] = [
        ("acerto_por_rodada", true, |m| Some(m.acerto_por_rodada)),
        ("nota_sequencia", true, |m| {
            m.nota_sequencia_por_rodada.faixa().copied()
        }),
        ("latencia_ms", false, |m| Some(m.latencia_ms)),
        ("tokens_por_s", true, |m| m.tokens_por_s.faixa().copied()),
        ("cpu_s_modelo", false, |m| m.cpu_s_modelo.faixa().copied()),
    ];
    if r.len() < 2 {
        return vec![];
    }
    metricas
        .iter()
        .map(|(nome, maior, pega)| {
            let elegiveis: Vec<&ResultadoModelo> = r
                .iter()
                .filter(|m| {
                    matches!(*nome, "acerto_por_rodada" | "nota_sequencia") || m.falhas == 0
                })
                .collect();
            let fora = r.len() - elegiveis.len();
            let com_falha = if fora > 0 {
                format!("; {fora} modelo(s) com falha fora da comparação")
            } else {
                String::new()
            };
            let (modelo, motivo) = if elegiveis.len() < 2 {
                (
                    None,
                    format!("menos de dois modelos comparáveis{com_falha}"),
                )
            } else if elegiveis.iter().any(|m| pega(m).is_none()) {
                (None, format!("não medida em todos{com_falha}"))
            } else {
                let v = elegiveis.iter().find_map(|cand| {
                    let fc = pega(cand)?;
                    elegiveis
                        .iter()
                        .filter(|o| o.modelo != cand.modelo)
                        .all(|o| {
                            pega(o)
                                .is_some_and(|fo| faixas_decidem(&fc, &fo, *maior) == Some(Lado::A))
                        })
                        .then(|| cand.modelo.clone())
                });
                let m = if v.is_some() {
                    "faixa separada de todas"
                } else {
                    "faixas se cruzam"
                };
                (v, format!("{m}{com_falha}"))
            };
            Vencedor {
                metrica: nome.to_string(),
                modelo,
                motivo,
            }
        })
        .collect()
}

/// A tabela do terminal: cada numero com a faixa, o N e a data.
pub fn tabela(a: &Avaliacao) -> String {
    let f = |m: &Medida, casas: usize| match m {
        Medida::Faixa(x) => format!(
            "{:.c$} [{:.c$}–{:.c$}] N={}",
            x.mediana,
            x.min,
            x.max,
            x.n,
            c = casas
        ),
        Medida::NaoMedida(motivo) => format!("não medida ({motivo})"),
    };
    let mut s = format!(
        "avaliacao de {} — {} caso(s) x {} rodada(s)\n",
        a.data, a.casos, a.rodadas
    );
    for m in &a.modelos {
        s.push_str(&format!(
            "\n{}  (N={} execucoes)\n  latencia ms : p50 {:.0}  p95 {}  [{:.0}–{:.0}]\n  tokens/s    : {}\n  CPU s agente: {}\n  CPU s modelo: {}\n  energia J   : {}\n  acerto      : {:.0}% por rodada [{:.0}%–{:.0}%] N={} rodada(s); {:.2}% no perfil\n",
            m.modelo,
            m.execucoes,
            m.p50_ms,
            m.p95_ms,
            m.latencia_ms.min,
            m.latencia_ms.max,
            f(&m.tokens_por_s, 1),
            f(&m.cpu_s_agente, 2),
            f(&m.cpu_s_modelo, 2),
            f(&m.energia_j, 3),
            m.acerto_por_rodada.mediana * 100.0,
            m.acerto_por_rodada.min * 100.0,
            m.acerto_por_rodada.max * 100.0,
            m.acerto_por_rodada.n,
            m.acerto_bp as f64 / 100.0,
        ));
        s.push_str(&format!(
            "  ferramentas : conjunto {} | sequencia {} (nota 0–1, mediana dos casos por rodada)\n",
            f(&m.nota_conjunto_por_rodada, 2),
            f(&m.nota_sequencia_por_rodada, 2),
        ));
        if m.falhas > 0 {
            s.push_str(&format!(
                "  FALHAS      : {}/{} execucoes; primeira: {}\n",
                m.falhas,
                m.execucoes,
                m.primeira_falha.as_deref().unwrap_or_default()
            ));
        }
    }
    if !a.por_prompt.is_empty() {
        s.push_str("\npor prompt (sha256 do sistema / das skills; compare antes/depois so dentro do mesmo):\n");
        for g in &a.por_prompt {
            let curto = |x: &Option<String>| {
                x.as_deref()
                    .map(|h| h.chars().take(12).collect::<String>())
                    .unwrap_or_else(|| "sem gravacao".into())
            };
            s.push_str(&format!(
                "  {:<24} prompt {} skills {}  acerto {}/{}  sequencia {}\n",
                g.modelo,
                curto(&g.prompt_sha256),
                curto(&g.skills_sha256),
                g.acertos,
                g.execucoes,
                f(&g.nota_sequencia, 2),
            ));
        }
    }
    if !a.vencedores.is_empty() {
        s.push_str("\nvencedor (so quando as faixas min–max nao se cruzam):\n");
        for v in &a.vencedores {
            s.push_str(&format!(
                "  {:<18} {}\n",
                v.metrica,
                match &v.modelo {
                    Some(m) => format!("{m} ({})", v.motivo),
                    None => format!("sem vencedor ({})", v.motivo),
                }
            ));
        }
    }
    s
}

#[cfg(test)]
mod testes {
    use super::*;

    fn fx(min: f64, max: f64) -> Faixa {
        Faixa {
            n: 3,
            min,
            mediana: (min + max) / 2.0,
            max,
        }
    }

    #[test]
    fn so_ha_vencedor_com_faixas_separadas() {
        assert_eq!(
            faixas_decidem(&fx(1.0, 2.0), &fx(3.0, 4.0), true),
            Some(Lado::B)
        );
        assert_eq!(
            faixas_decidem(&fx(1.0, 2.0), &fx(3.0, 4.0), false),
            Some(Lado::A)
        );
        // Encostar e cruzar.
        assert_eq!(faixas_decidem(&fx(1.0, 3.0), &fx(3.0, 4.0), true), None);
        // Mediana muito melhor dentro do ruido nao e vitoria.
        assert_eq!(faixas_decidem(&fx(0.0, 10.0), &fx(9.0, 9.5), true), None);
    }

    #[test]
    fn percentil_por_posto_bate_com_o_do_ai_benchmark() {
        let v: Vec<f64> = (1..=20).map(f64::from).collect();
        assert_eq!(percentil(&v, 50), 10.0);
        assert_eq!(percentil(&v, 95), 19.0);
        assert_eq!(percentil(&[7.0], 95), 7.0);
    }

    #[test]
    fn cpu_le_o_stat_com_parenteses_no_nome() {
        let d = std::env::temp_dir().join(format!("phx-cpu-{}", uuid::Uuid::now_v7()));
        std::fs::create_dir_all(&d).unwrap();
        // comm com espaco e parentese: o corte e no ULTIMO `)`.
        std::fs::write(
            d.join("stat"),
            "42 (a b) c) S 1 1 1 0 -1 0 0 0 0 0 250 50 30 20 20 0 1 0 1 1 1",
        )
        .unwrap();
        assert_eq!(cpu_s(&d, false), Some(3.0));
        assert_eq!(cpu_s(&d, true), Some(3.5));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn cpu_do_modelo_conta_o_filho_de_qualquer_nome() {
        let d = std::env::temp_dir().join(format!("phx-proc-{}", uuid::Uuid::now_v7()));
        let proc = |pid: u32, ppid: u32, comm: &str, utime: u64| {
            let p = d.join(pid.to_string());
            std::fs::create_dir_all(&p).unwrap();
            std::fs::write(p.join("comm"), format!("{comm}\n")).unwrap();
            std::fs::write(
                p.join("stat"),
                format!("{pid} ({comm}) S {ppid} 1 1 0 -1 0 0 0 0 0 {utime} 0 0 0 20 0"),
            )
            .unwrap();
        };
        // O neto aparece ANTES do pai na listagem: a descida nao pode depender da ordem.
        proc(5, 11, "neto", 100);
        proc(10, 1, "ollama", 120);
        proc(11, 10, "llama-server", 35_000);
        proc(12, 1, "bash", 999);
        let m = cpu_da_arvore(&d, "ollama");
        assert_eq!(m.keys().copied().collect::<Vec<_>>(), [5, 10, 11]);
        assert!((m.values().sum::<f64>() - 352.2).abs() < 1e-9);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn nvidia_soma_por_gpu_e_recusa_parcial() {
        assert_eq!(somar_nvidia("1000\n2500\n"), Some(3500));
        assert_eq!(somar_nvidia("1000\n[N/A]\n"), None);
        assert_eq!(somar_nvidia(""), None);
    }

    /// SP000030: a nota parcial e deterministica e distingue «chamou as certas fora de
    /// ordem» (conjunto 1, sequencia < 1) de «faltou uma» (as duas < 1). Reposta a nota
    /// so por acerto exato, os tres casos abaixo dariam o mesmo 0.
    #[test]
    fn nota_de_ferramentas_conjunto_e_lcs() {
        let v = |xs: &[&str]| xs.iter().map(|x| x.to_string()).collect::<Vec<_>>();
        let esp = v(&["ler", "editar", "testar"]);
        let n = nota_ferramentas(&esp, &esp).unwrap();
        assert_eq!((n.conjunto, n.sequencia), (1.0, 1.0));
        // Mesmo conjunto, ordem trocada: conjunto inteiro, sequencia 2/3.
        let n = nota_ferramentas(&esp, &v(&["editar", "ler", "testar"])).unwrap();
        assert_eq!((n.conjunto, n.sequencia), (1.0, 2.0 / 3.0));
        // Faltou uma, e uma a mais no meio: 2/3 nas duas.
        let n = nota_ferramentas(&esp, &v(&["ler", "listar", "testar"])).unwrap();
        assert_eq!((n.conjunto, n.sequencia), (2.0 / 3.0, 2.0 / 3.0));
        // Repeticao nao conta duas vezes; nada chamado e 0; sem gabarito nao ha nota.
        let n = nota_ferramentas(&esp, &v(&["ler", "ler", "ler"])).unwrap();
        assert_eq!((n.conjunto, n.sequencia), (1.0 / 3.0, 1.0 / 3.0));
        assert_eq!(nota_ferramentas(&esp, &[]).unwrap().sequencia, 0.0);
        assert_eq!(nota_ferramentas(&[], &esp), None);
        assert_eq!(lcs(&v(&["a", "b", "c", "d"]), &v(&["b", "d", "a", "c"])), 2);
    }

    #[test]
    fn tokens_por_s_so_com_toda_resposta_medida() {
        let g = |t, ns| Geracao { tokens: t, ns };
        assert_eq!(
            tokens_por_s(&[g(50, Some(500_000_000)), g(50, Some(500_000_000))]),
            Some(100.0)
        );
        assert_eq!(tokens_por_s(&[g(50, Some(1)), g(50, None)]), None);
    }
}
