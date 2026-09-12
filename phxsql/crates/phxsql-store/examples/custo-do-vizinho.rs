//! Bancada da premissa vetorial (Frente V, item V2 -- `docs/BACKLOG-TIPOS.md`,
//! `docs/propostas/vetorial.md` §1, `docs/VETORES.md` §5): forca bruta
//! escalar basta para K-NN, ou o vetorial precisa nascer com um indice ANN?
//!
//! ```bash
//! cargo build --release --example custo-do-vizinho -p phxsql-store
//! target/release/examples/custo-do-vizinho              # sweep completo
//! target/release/examples/custo-do-vizinho 100000 768   # 1 combinacao (calibragem)
//! target/release/examples/custo-do-vizinho 100000 768 41 # ... com M consultas
//! ```
//!
//! Gera N vetores f32 de dimensao `d` (gerador PROPRIO, deterministico --
//! regra 1 da bancada, `bancada/LEIA-ME.md`: "mesmos dados, sem sorteio"),
//! roda M consultas K-NN (K=10, cosseno) por FORCA BRUTA LINEAR -- varre os N,
//! calcula N distancias com o nucleo zero-dep de `phxsql_core::vetor` (frente
//! V1), mantem os 10 melhores -- e mede a MEDIANA e a FAIXA min-max da
//! latencia por consulta. Sweep: N em {10 mil, 100 mil, 1 milhao} x d em
//! {384, 768, 1536}.
//!
//! # O que NAO e medido aqui
//!
//! Isto e o NUCLEO (V1) exercitado em forca bruta, nao o motor: sem
//! `ColumnType::Vetor`, sem arquivo `.vec`, sem nada em disco -- o corpus vive
//! so em memoria, do mesmo jeito que `onde-doi.rs` mede o `.reg` isolado do
//! resto. O numero que sai daqui e o VEREDITO da premissa: se a forca bruta ja
//! responde rapido o bastante para um uso interativo, o tipo vetorial nasce
//! sem indice novo -- e sem `.hnsw`/`.ivf`. Grava
//! `bancada/vetorial/resultados.json`, no molde dos outros `resultados.json`
//! da casa (mediana + faixa min-max, `maquina`, `medido_em`).
//!
//! # Por que as normas do corpus saem PRE-computadas
//!
//! Chamar `similaridade_cosseno` (que recalcula `norma()` dos DOIS lados a
//! cada chamada) recalcularia a norma do MESMO vetor do corpus em toda
//! consulta -- N vezes por consulta, M vezes por vetor. Isso mediria a
//! REPETICAO da conta, nao a busca: qualquer motor real guardaria a norma
//! junto do vetor no INSERT (e um `f32` a mais por linha, nao um indice). Por
//! isso o corpus pre-computa `norma()` uma vez por vetor na carga, e o laco
//! quente so chama `produto_interno` (a parte que realmente cresce com N) mais
//! uma divisao por duas normas ja prontas -- e o nucleo V1 continua sendo o
//! que faz a conta, sem contornar `assert_eq!` nem reimplementar a formula.
//!
//! # A regra 4 da bancada (mesmo trabalho, mesmo resultado)
//!
//! Nao ha "outro motor" aqui para comparar -- e a mesma bancada da grade
//! ordenada (`o-que-a-grade-ordenada-custa.rs`, pedido 188): os DOIS lados
//! (K=10 pequeno x K=10 grande, ou N pequeno x N grande) fazem o MESMO
//! trabalho -- calcular todas as N distancias e manter os K melhores --, e a
//! unica variavel entre combinacoes e o que o sweep declara (N e d). Nenhuma
//! combinacao ganha atalho que a outra nao tem.

use std::io::Write;
use std::time::Instant;

use phxsql_core::json::Json;
use phxsql_core::vetor::{norma, produto_interno};

/// splitmix64 -- a mesma receita de `phxsql_core::uuid.rs` (nao
/// criptografico, so para dado sintetico deterministico e repetivel: mesma
/// seed, mesmos vetores, sempre).
struct Splitmix64(u64);

impl Splitmix64 {
    fn novo(seed: u64) -> Self {
        Splitmix64(seed)
    }

    fn proximo(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Um `f32` em `[-1.0, 1.0)` -- a faixa tipica de um embedding.
    fn proximo_f32(&mut self) -> f32 {
        let bits = (self.proximo() >> 40) as u32; // 24 bits, exatos num f32
        (bits as f32 / (1u32 << 24) as f32) * 2.0 - 1.0
    }
}

/// N vetores de dimensao `d`, lado a lado num `Vec<f32>` so -- sem o
/// indireto de `Vec<Vec<f32>>`, no mesmo layout "linhas de tamanho fixo, uma
/// apos a outra" que um `.vec` em disco teria (mas isto e so memoria: ver o
/// aviso no topo do arquivo).
struct Corpus {
    d: usize,
    dados: Vec<f32>,
    /// Uma norma por vetor, pre-computada na carga (ver o comentario do
    /// topo do arquivo sobre por que ela NAO entra no laco quente).
    normas: Vec<f32>,
}

impl Corpus {
    fn gerar(n: usize, d: usize, seed: u64) -> Corpus {
        let mut rng = Splitmix64::novo(seed);
        let mut dados = Vec::with_capacity(n * d);
        for _ in 0..n * d {
            dados.push(rng.proximo_f32());
        }
        let mut normas = Vec::with_capacity(n);
        for i in 0..n {
            normas.push(norma(&dados[i * d..(i + 1) * d]));
        }
        Corpus { d, dados, normas }
    }

    fn vetor(&self, i: usize) -> &[f32] {
        &self.dados[i * self.d..(i + 1) * self.d]
    }

    fn n(&self) -> usize {
        self.normas.len()
    }
}

/// Os K menores mantidos por insercao ordenada. K=10 e pequeno demais para
/// justificar um `BinaryHeap`: o vetor fica sempre ordenado, e so vetores que
/// ja batem o pior dos K pagam o `insert`.
struct TopK {
    k: usize,
    itens: Vec<(f32, usize)>,
}

impl TopK {
    fn novo(k: usize) -> Self {
        TopK {
            k,
            itens: Vec::with_capacity(k + 1),
        }
    }

    fn considerar(&mut self, dist: f32, idx: usize) {
        if self.itens.len() == self.k && dist >= self.itens[self.k - 1].0 {
            return; // pior que o pior dos K -- nem entra
        }
        let pos = self.itens.partition_point(|&(d, _)| d <= dist);
        self.itens.insert(pos, (dist, idx));
        if self.itens.len() > self.k {
            self.itens.pop();
        }
    }
}

/// Uma consulta K-NN por forca bruta: varre os N vetores do corpus, calcula N
/// distancias por cosseno (produto interno cru + as duas normas -- a do
/// corpus PRE-computada, a da consulta calculada uma vez aqui) e mantem os
/// `k` melhores.
fn knn_forca_bruta(corpus: &Corpus, consulta: &[f32], k: usize) -> Vec<(f32, usize)> {
    let nq = norma(consulta);
    let mut topk = TopK::novo(k);
    for i in 0..corpus.n() {
        let ni = corpus.normas[i];
        let sim = if nq == 0.0 || ni == 0.0 {
            0.0
        } else {
            produto_interno(consulta, corpus.vetor(i)) / (nq * ni)
        };
        topk.considerar(1.0 - sim, i);
    }
    topk.itens
}

fn mediana(mut v: Vec<f64>) -> f64 {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    v[v.len() / 2]
}

struct Medida {
    n: usize,
    d: usize,
    consultas: usize,
    mediana_ms: f64,
    faixa_ms: (f64, f64),
}

fn medir_combinacao(n: usize, d: usize, m: usize, k: usize, seed: u64) -> Medida {
    print!("  N={n:>8} d={d:>5} (M={m:>3}) ... ");
    std::io::stdout().flush().ok();

    let corpus = Corpus::gerar(n, d, seed);

    // As M consultas saem de outro bloco do MESMO gerador, montadas ANTES do
    // cronometro -- gerar a consulta nao e o que se quer medir.
    let mut rng = Splitmix64::novo(seed ^ 0xC0FF_EE00_C0FF_EE00);
    let consultas: Vec<Vec<f32>> = (0..m)
        .map(|_| (0..d).map(|_| rng.proximo_f32()).collect())
        .collect();

    let mut tempos = Vec::with_capacity(m);
    let mut ancora = 0usize; // prova de que o resultado do laco foi usado
    let esperado = k.min(n);
    for q in &consultas {
        let inicio = Instant::now();
        let top = knn_forca_bruta(&corpus, q, k);
        tempos.push(inicio.elapsed().as_secs_f64() * 1000.0);
        assert_eq!(top.len(), esperado, "K-NN nao devolveu os K melhores");
        ancora ^= top[0].1;
    }
    std::hint::black_box(ancora);

    let min = tempos.iter().cloned().fold(f64::INFINITY, f64::min);
    let max = tempos.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let med = mediana(tempos);
    println!("mediana {med:>8.3} ms   faixa [{min:>8.3}; {max:>8.3}] ms");

    Medida {
        n,
        d,
        consultas: m,
        mediana_ms: med,
        faixa_ms: (min, max),
    }
}

/// Custo cru de UMA distancia de dimensao `d` -- produto interno + as duas
/// normas, isolado do corpus e do `TopK`. E o piso que o `knn_forca_bruta`
/// paga N vezes por consulta.
fn medir_uma_distancia(d: usize, seed: u64) -> f64 {
    let mut rng = Splitmix64::novo(seed);
    let a: Vec<f32> = (0..d).map(|_| rng.proximo_f32()).collect();
    let b: Vec<f32> = (0..d).map(|_| rng.proximo_f32()).collect();
    let voltas = 100_000u32;
    let inicio = Instant::now();
    let mut soma = 0f64;
    for _ in 0..voltas {
        let na = norma(std::hint::black_box(&a));
        let nb = norma(std::hint::black_box(&b));
        let pi = produto_interno(std::hint::black_box(&a), std::hint::black_box(&b));
        soma += (pi / (na * nb)) as f64;
    }
    std::hint::black_box(soma);
    inicio.elapsed().as_secs_f64() * 1e6 / voltas as f64 // us
}

/// Descricao da maquina e a carga no INICIO da corrida -- nao para provar
/// "maquina parada" (isso e responsabilidade de quem chama, junto do
/// `bancada/esta-medindo.sh` da casa), mas para o numero carregar a condicao
/// em que saiu, em vez de a mediana viajar sozinha.
fn maquina_info() -> (String, f64) {
    let nucleos = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(0);
    let carga1min = std::fs::read_to_string("/proc/loadavg")
        .ok()
        .and_then(|s| s.split_whitespace().next().map(str::to_string))
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(-1.0);
    (
        format!("container Linux, {nucleos} nucleos, escalar (sem SIMD/threads)"),
        carga1min,
    )
}

fn agora_utc_min() -> String {
    // Sem `chrono` (zero deps): so a data corrida em UTC, no formato
    // "AAAA-MM-DD HH:MM" que os outros `resultados.json` da casa usam.
    // `date -u` e o mesmo comando que `bancada/*.py` chamaria por baixo.
    std::process::Command::new("date")
        .args(["-u", "+%Y-%m-%d %H:%M"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "desconhecido".to_string())
}

fn arred(x: f64, casas: i32) -> f64 {
    let f = 10f64.powi(casas);
    (x * f).round() / f
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let k = 10usize;
    let seed = 0x5EED_1234_ABCDu64;

    println!("=== custo-do-vizinho: K-NN por forca bruta linear, K={k}, cosseno ===\n");

    // Uma combinacao so (calibragem) se vier N e d na linha de comando;
    // senao, o sweep completo do pedido: N x {1e4,1e5,1e6} x d x {384,768,1536}.
    let (combinacoes, sweep_completo): (Vec<(usize, usize, usize)>, bool) = if args.len() >= 3 {
        let n: usize = args[1].parse().expect("N invalido");
        let d: usize = args[2].parse().expect("d invalido");
        let m: usize = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(21);
        (vec![(n, d, m)], false)
    } else {
        let mut v = vec![];
        for &n in &[10_000usize, 100_000, 1_000_000] {
            // M menor nas combinacoes maiores -- cada consulta ja custa
            // mais (N cresce), e o QUE se mede (mediana e faixa) nao muda
            // de natureza com menos amostras; so a faixa fica um pouco
            // mais estreita. Documentado aqui, nao escondido no numero.
            let m = if n >= 1_000_000 { 11 } else { 21 };
            for &d in &[384usize, 768, 1536] {
                v.push((n, d, m));
            }
        }
        (v, true)
    };

    println!("custo cru de UMA distancia, por dimensao:");
    let mut distancia_pura_us: Vec<(usize, f64)> = vec![];
    for &d in &[384usize, 768, 1536] {
        let us = medir_uma_distancia(d, seed ^ d as u64);
        println!("  d={d:>5} ... {us:>8.3} us/distancia");
        distancia_pura_us.push((d, us));
    }
    println!();

    let mut medidas = vec![];
    for (n, d, m) in combinacoes {
        medidas.push(medir_combinacao(n, d, m, k, seed));
    }

    // --------------------------------------------------------- o veredito
    // Criterio de "interativo": ate 50 ms por consulta -- o mesmo piso que
    // uma UI aceita para uma busca digitada sem sentir trava (a mesma ordem
    // de grandeza do que esta casa ja mede como "rapido" numa tela: um
    // `varrer` pela rede fica na casa de poucos ms). Acima disso, a resposta
    // ainda "funciona", mas deixa de ser instantanea para quem espera.
    const LIMITE_INTERATIVO_MS: f64 = 50.0;
    let pior_ate_o_limite = medidas
        .iter()
        .filter(|m| m.mediana_ms <= LIMITE_INTERATIVO_MS)
        .max_by(|a, b| (a.n, a.d).cmp(&(b.n, b.d)));
    let primeiro_acima = medidas
        .iter()
        .filter(|m| m.mediana_ms > LIMITE_INTERATIVO_MS)
        .min_by(|a, b| (a.n, a.d).cmp(&(b.n, b.d)));

    println!(
        "\n=== veredito da premissa (criterio: <= {LIMITE_INTERATIVO_MS:.0} ms/consulta) ===\n"
    );
    for m in &medidas {
        let veredito = if m.mediana_ms <= LIMITE_INTERATIVO_MS {
            "forca bruta basta"
        } else {
            "PRECISA de ANN"
        };
        println!(
            "  N={:>8} d={:>5} -> {:>8.3} ms  [{}]",
            m.n, m.d, m.mediana_ms, veredito
        );
    }
    if let Some(m) = pior_ate_o_limite {
        println!(
            "\n  Maior combinacao medida ainda dentro do limite: N={} d={} ({:.3} ms).",
            m.n, m.d, m.mediana_ms
        );
    }
    if let Some(m) = primeiro_acima {
        println!(
            "  Primeira combinacao medida FORA do limite: N={} d={} ({:.3} ms).",
            m.n, m.d, m.mediana_ms
        );
    } else {
        println!("\n  Nenhuma combinacao medida passou do limite -- forca bruta bastou em TODAS.");
    }

    // ------------------------------------------------------------- grava
    let (maquina, carga1min) = maquina_info();
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../bancada/vetorial");
    std::fs::create_dir_all(&dir).ok();
    let alvo = dir.join("resultados.json");

    let combinacoes_json: Vec<Json> = medidas
        .iter()
        .map(|m| {
            Json::objeto(vec![
                ("n", Json::de_u64(m.n as u64)),
                ("d", Json::de_u64(m.d as u64)),
                ("consultas", Json::de_u64(m.consultas as u64)),
                ("distancias_por_consulta", Json::de_u64(m.n as u64)),
                ("latencia_mediana_ms", Json::Numero(arred(m.mediana_ms, 4))),
                (
                    "latencia_faixa_ms",
                    Json::Lista(vec![
                        Json::Numero(arred(m.faixa_ms.0, 4)),
                        Json::Numero(arred(m.faixa_ms.1, 4)),
                    ]),
                ),
                (
                    "veredito",
                    Json::texto_de(if m.mediana_ms <= LIMITE_INTERATIVO_MS {
                        "forca_bruta_basta"
                    } else {
                        "precisa_de_ann"
                    }),
                ),
            ])
        })
        .collect();

    let distancia_pura_json: Vec<Json> = distancia_pura_us
        .iter()
        .map(|&(d, us)| {
            Json::objeto(vec![
                ("d", Json::de_u64(d as u64)),
                ("us_por_distancia", Json::Numero(arred(us, 4))),
            ])
        })
        .collect();

    let raiz = Json::objeto(vec![
        ("bancada", Json::texto_de("custo-do-vizinho")),
        ("metrica", Json::texto_de("cosseno")),
        ("k", Json::de_u64(k as u64)),
        ("sweep_completo", Json::de_bool(sweep_completo)),
        ("limite_interativo_ms", Json::Numero(LIMITE_INTERATIVO_MS)),
        ("distancia_pura", Json::Lista(distancia_pura_json)),
        ("combinacoes", Json::Lista(combinacoes_json)),
        ("maquina", Json::texto_de(maquina)),
        ("carga_1min_no_inicio", Json::Numero(carga1min)),
        (
            "aviso_da_maquina",
            Json::texto_de(
                "corpus e consultas em memoria, escalar (sem SIMD); numero e o \
                 PISO de uma implementacao ingenua, nao o teto de uma otimizada",
            ),
        ),
        ("medido_em", Json::texto_de(agora_utc_min())),
    ]);

    std::fs::write(&alvo, raiz.escrever_identado()).expect("gravar resultados.json");
    println!("\ngravado em {}", alvo.display());
}
