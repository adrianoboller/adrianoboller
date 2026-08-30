//! Onde uma GPU ajudaria neste motor -- e onde a conta do barramento a mata.
//!
//! ```bash
//! cargo build --release --examples -p phxsql-store   # o binario do medidor
//! cargo run --release --example onde-a-gpu-ajudaria -- [linhas]
//! ```
//!
//! Existe por causa da regra da casa: *receita de fora se mede contra o nosso
//! gargalo antes de virar plano*. O pedido foi «ativar GPU CUDA para ajudar no
//! processamento pesado», e a pergunta que vem antes dele e uma so: **onde
//! esta o processamento pesado, e ele e do tipo que uma GPU acelera?**
//!
//! GPU ganha em trabalho **aritmetico, uniforme, sobre muitos dados, com pouca
//! dependencia entre eles**. Ela perde quando o trabalho e ponteiro atras de
//! ponteiro, quando ramifica a cada passo, ou quando o dado tem de atravessar
//! um barramento para chegar la. Este medidor separa as tres coisas:
//!
//! 1. **o teto da maquina** -- a banda de memoria, que e o limite de qualquer
//!    laco que so passa por bytes. Nenhum nucleo aritmetico pode ser mais
//!    rapido do que a RAM entrega, e a copia para a GPU tambem le daqui;
//! 2. **os nucleos aritmeticos como estao escritos hoje** -- CRC-32, SHA-256 e
//!    ChaCha20-Poly1305, em MB/s, que sao os candidatos plausiveis;
//! 3. **quanto desse trabalho existe de verdade** numa insercao, numa
//!    varredura, numa ordenacao e numa busca por chave -- porque acelerar 1%
//!    de uma operacao em 100x ainda deixa 99% no lugar.
//!
//! No fim ele faz **a conta da travessia**, que e a que decide. Ela e dada de
//! presente para a GPU: nucleo de custo **zero**, volta de custo **zero**,
//! barramento no **pico teorico**. Se com tudo isso ela ainda perde, perde.
//!
//! O que este medidor NAO mede: PCIe, porque **nao ha GPU nesta maquina**
//! (sem `/dev/nvidia*`, sem `nvcc`, sem `nvidia-smi`). Os numeros do
//! barramento entram como **especificacao declarada**, nao como medida -- e
//! estao marcados assim no relatorio, porque numero citado e numero que
//! ninguem mediu.

use std::time::Instant;

use phxsql_core::cifra::{self, CHAVE_LEN, NONCE_LEN};
use phxsql_core::crc::crc32;
use phxsql_core::hash::sha256;
use phxsql_core::paralelo::nucleos;
use phxsql_core::schema::{Column, IndexColumn, IndexDef, Schema};
use phxsql_core::types::ColumnType;
use phxsql_core::value::Value;
use phxsql_store::memoria::{Consulta, Filtro, Operador, Ordem, TabelaMemoria};
use phxsql_store::table::Table;

const CIDADES: [&str; 8] = [
    "Blumenau",
    "Joinville",
    "Itajai",
    "Curitiba",
    "Chapeco",
    "Lages",
    "Florianopolis",
    "Criciuma",
];

/// Pico **teorico** de cada geracao de PCIe, x16, em MB/s (10^6 bytes/s nao --
/// aqui e MiB/s, para casar com o resto do relatorio).
///
/// Sao numeros de **especificacao do barramento**, nao medidas desta maquina:
/// nao ha GPU aqui para medir. Entram no pico, e nao na vazao real (que fica
/// entre 75% e 85% dele com memoria travada), porque o objetivo e dar a melhor
/// chance possivel a GPU: se ela perde com o barramento no teto, perde sempre.
const BARRAMENTOS: [(&str, f64); 3] = [
    ("PCIe 3.0 x16", 15_754.0),
    ("PCIe 4.0 x16", 31_508.0),
    ("PCIe 5.0 x16", 63_015.0),
];

/// Ida e volta de um nucleo CUDA vazio: lancamento mais sincronizacao.
///
/// Tambem e especificacao (a faixa publicada e de 5 a 20 us em maquina
/// dedicada); entra no piso da faixa pelo mesmo motivo do barramento.
const LATENCIA_LANCAMENTO_US: f64 = 5.0;

fn mibs(bytes: u64, segundos: f64) -> f64 {
    (bytes as f64 / 1_048_576.0) / segundos
}

/// Repete ate passar de `minimo` segundos, para o relogio nao virar o ruido.
///
/// Devolve (segundos por volta, voltas, sumidouro).
///
/// # Por que `black_box`, e o que ele conserta
///
/// A primeira versao deste medidor deu **12,5 bilhoes de MiB/s** de banda de
/// memoria e **192 milhoes de MiB/s** de CRC-32. Nao ha maquina assim: o
/// otimizador viu que `crc32(&pagina)` e funcao pura de um `slice` que nao
/// muda dentro do laco, calculou uma vez e ergueu a chamada para fora. O laco
/// media um laco vazio.
///
/// `black_box` fecha os dois lados: a entrada volta opaca a cada volta, entao
/// a chamada nao pode ser erguida; e a saida e consumida, entao a chamada nao
/// pode ser apagada por ninguem usar o resultado. Medidor com furo mede o
/// furo, e este tinha um.
fn medir(minimo: f64, mut passo: impl FnMut(u64) -> u64) -> (f64, u64, u64) {
    let mut voltas = 0u64;
    let mut sumidouro = 0u64;
    let inicio = Instant::now();
    loop {
        for _ in 0..8 {
            sumidouro = sumidouro.wrapping_add(std::hint::black_box(passo(voltas)));
            voltas += 1;
        }
        if inicio.elapsed().as_secs_f64() >= minimo {
            break;
        }
    }
    let s = inicio.elapsed().as_secs_f64();
    (s / voltas as f64, voltas, sumidouro)
}

/// A melhor de `n` corridas. A maquina desta sessao roda outros processos nos
/// mesmos quatro nucleos: a mediana de uma corrida so mediria a vizinhanca.
fn melhor_de(n: usize, mut corrida: impl FnMut() -> f64) -> f64 {
    let mut melhor = f64::MAX;
    for _ in 0..n {
        melhor = melhor.min(corrida());
    }
    melhor
}

fn colunas() -> Vec<Column> {
    vec![
        Column::new("id", ColumnType::Int8).obrigatoria(),
        Column::new("produto", ColumnType::Str(40)).obrigatoria(),
        Column::new("cidade", ColumnType::Str(20)),
        Column::new("valor", ColumnType::Int8),
    ]
}

fn linha(i: i64) -> Vec<Value> {
    vec![
        Value::Int(i),
        Value::Str(format!("Produto {i:08}")),
        Value::Str(CIDADES[(i as usize) % CIDADES.len()].into()),
        Value::Int(i % 100_000),
    ]
}

/// Um candidato a GPU: o que ele e, quantos MiB/s a CPU faz hoje, e quanto
/// desse trabalho existe na operacao real.
struct Candidato {
    nome: &'static str,
    mibs_cpu: f64,
    /// Quanto da operacao real este nucleo ocupa, em %, quando ela acontece.
    peso: String,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let n: i64 = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(1_000_000);

    println!("=== onde a GPU ajudaria: o que esta maquina faz hoje ===\n");
    println!(
        "  nucleos disponiveis para o trabalho dividido: {}",
        nucleos()
    );
    println!("  linhas do ensaio: {n}\n");

    let mut candidatos: Vec<Candidato> = Vec::new();

    // ---------------------------------------------------------------- 1
    // O teto. Todo laco que so passa por bytes esbarra aqui, e a copia para a
    // GPU tambem le daqui: ela nao escapa deste numero, ela o paga de novo.
    println!("=== 1. O teto da maquina: a banda de memoria ===\n");

    const BLOCO: usize = 64 * 1024 * 1024;
    let fonte: Vec<u8> = (0..BLOCO).map(|i| (i % 251) as u8).collect();
    let mut destino = vec![0u8; BLOCO];

    let copia = melhor_de(3, || {
        let (s, _, _) = medir(0.3, |_| {
            destino.copy_from_slice(std::hint::black_box(&fonte));
            std::hint::black_box(&destino)[0] as u64
        });
        mibs(BLOCO as u64, s)
    });

    let leitura = melhor_de(3, || {
        let (s, _, _) = medir(0.3, |_| {
            // Soma em u64 por palavra: e o laco mais barato que ainda toca
            // todo byte, entao o que ele mede e a entrega da RAM.
            let mut acc = 0u64;
            for pedaco in std::hint::black_box(&fonte).chunks_exact(8) {
                acc = acc.wrapping_add(u64::from_le_bytes(pedaco.try_into().unwrap()));
            }
            acc
        });
        mibs(BLOCO as u64, s)
    });

    println!("  copiar 64 MiB (le 64, escreve 64) ....... {copia:>9.0} MiB/s");
    println!("  somar 64 MiB (so leitura) ............... {leitura:>9.0} MiB/s");
    println!(
        "\n  A leitura pura e o teto de qualquer nucleo que passe por todo byte.\n  \
         Nenhum dos nucleos abaixo pode passar disto, e a travessia para a GPU\n  \
         teria de pagar esta leitura ANTES de chegar ao barramento.\n"
    );

    // ---------------------------------------------------------------- 2
    // Os nucleos aritmeticos. Sao os candidatos plausiveis: uniformes, sem
    // ramificacao dependente de dado, sobre muitos bytes.
    println!("=== 2. Os nucleos aritmeticos, como estao escritos hoje ===\n");

    let pagina: Vec<u8> = (0..4096).map(|i| (i % 251) as u8).collect();
    let mib: Vec<u8> = (0..1024 * 1024).map(|i| (i % 251) as u8).collect();

    let crc_pagina = melhor_de(3, || {
        let (s, _, _) = medir(0.3, |_| crc32(std::hint::black_box(&pagina)) as u64);
        mibs(pagina.len() as u64, s)
    });
    let crc_mib = melhor_de(3, || {
        let (s, _, _) = medir(0.3, |_| crc32(std::hint::black_box(&mib)) as u64);
        mibs(mib.len() as u64, s)
    });
    let sha_mib = melhor_de(3, || {
        let (s, _, _) = medir(0.3, |_| sha256(std::hint::black_box(&mib))[0] as u64);
        mibs(mib.len() as u64, s)
    });

    let chave = [7u8; CHAVE_LEN];
    let nonce = [3u8; NONCE_LEN];
    let cifra_pagina = melhor_de(3, || {
        let (s, _, _) = medir(0.3, |_| {
            let (c, t) = cifra::selar(&chave, &nonce, b"", std::hint::black_box(&pagina));
            c[0] as u64 + t[0] as u64
        });
        mibs(pagina.len() as u64, s)
    });
    let cifra_mib = melhor_de(3, || {
        let (s, _, _) = medir(0.3, |_| {
            let (c, t) = cifra::selar(&chave, &nonce, b"", std::hint::black_box(&mib));
            c[0] as u64 + t[0] as u64
        });
        mibs(mib.len() as u64, s)
    });

    println!("  {:<44} {:>9}  % do teto de leitura", "nucleo", "MiB/s");
    println!("  {}", "-".repeat(78));
    for (nome, v) in [
        ("CRC-32 de uma pagina de 4 KiB (.ndx)", crc_pagina),
        ("CRC-32 de um bloco de 1 MiB", crc_mib),
        ("SHA-256 de um bloco de 1 MiB (backup)", sha_mib),
        ("ChaCha20-Poly1305 selar, pagina de 4 KiB", cifra_pagina),
        ("ChaCha20-Poly1305 selar, bloco de 1 MiB", cifra_mib),
    ] {
        println!("  {nome:<44} {v:>9.0}  {:>5.1}%", 100.0 * v / leitura);
    }
    println!();

    // ---------------------------------------------------------------- 3
    // Quanto desse trabalho existe de verdade. Um nucleo que ocupa 1% da
    // operacao nao muda a operacao nem sendo instantaneo -- e a lei de Amdahl
    // escrita com o numero desta casa.
    println!("=== 3. Quanto desse trabalho existe numa operacao real ===\n");

    let dir = std::env::temp_dir().join(format!("phx-gpu-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir)?;

    let indices = vec![
        IndexDef::new("pk", vec![IndexColumn::asc(0)]).unico(),
        IndexDef::new("por_cidade", vec![IndexColumn::asc(2)]),
    ];
    let esquema = Schema::new("precos", colunas(), indices)?;
    let mut t = Table::criar(&dir, esquema)?;

    let inicio = Instant::now();
    for i in 1..=n {
        t.inserir(&linha(i))?;
    }
    t.sincronizar()?;
    let s_inserir = inicio.elapsed().as_secs_f64();
    let us_por_linha = s_inserir * 1e6 / n as f64;

    // O CRC que uma insercao paga hoje: so as paginas GRAVADAS passam por ele,
    // porque o cache de leitura serve as outras sem recalcular (DESEMPENHO §2).
    // O `onde-doi` mede 0,02 pagina gravada por linha na forma de 2 indices.
    let paginas_gravadas_por_linha = 0.02_f64;
    let us_por_pagina = 4096.0 / (crc_pagina * 1.048576);
    let us_crc_por_linha = paginas_gravadas_por_linha * us_por_pagina;
    let pct_crc = 100.0 * us_crc_por_linha / us_por_linha;

    // O teto do mesmo item: se TODA pagina tocada pagasse CRC, como acontecia
    // antes do cache de write-back. Serve para dizer o quanto ele ja rendeu --
    // e para nao vender o 0,5% de hoje como se fosse uma propriedade eterna.
    let paginas_tocadas_por_linha = 8.82_f64;
    let pct_crc_teto = 100.0 * paginas_tocadas_por_linha * us_por_pagina / us_por_linha;

    println!("  insercao de {n} linhas, 2 indices: {s_inserir:.2}s, {us_por_linha:.2} us/linha");
    println!("  CRC-32 de uma pagina de 4 KiB: {us_por_pagina:.2} us");
    println!(
        "  dentro da insercao: {paginas_gravadas_por_linha} pagina gravada/linha \
         = {us_crc_por_linha:.3} us = {pct_crc:.2}% da insercao"
    );
    println!(
        "  -> mesmo um CRC INSTANTANEO deixaria a insercao em {:.2} us ({:.3}x)",
        us_por_linha - us_crc_por_linha,
        us_por_linha / (us_por_linha - us_crc_por_linha)
    );
    println!(
        "  (o teto do item: se as {paginas_tocadas_por_linha} paginas tocadas por linha \
         pagassem CRC, seriam {pct_crc_teto:.1}% --\n   \
         o cache de write-back ja comprou essa diferenca, sem GPU)\n"
    );

    candidatos.push(Candidato {
        nome: "CRC-32 de pagina, na insercao",
        mibs_cpu: crc_pagina,
        peso: format!("{pct_crc:.2}% de uma insercao"),
    });

    // A busca por chave: o contra-exemplo perfeito. Descer a B+tree e ponteiro
    // atras de ponteiro, com uma comparacao ramificada por nivel -- cada passo
    // PRECISA do resultado do anterior para saber onde ler o proximo.
    let mut achadas = 0usize;
    let buscas = 20_000i64;
    let s_buscar = melhor_de(3, || {
        let inicio = Instant::now();
        achadas = 0;
        for k in 0..buscas {
            let alvo = 1 + (k * 7919) % n;
            achadas += t.buscar("pk", &[Value::Int(alvo)]).unwrap().len();
        }
        inicio.elapsed().as_secs_f64()
    });
    println!(
        "  busca por chave: {buscas} descidas em {:.3}s = {:.2} us cada ({achadas} achadas)",
        s_buscar,
        s_buscar * 1e6 / buscas as f64
    );
    println!(
        "  -> zero bytes de aritmetica uniforme: e uma cadeia de dependencia por\n     \
         nivel da arvore. Nao ha o que dividir entre 10.000 threads.\n"
    );

    // ---------------------------------------------------------------- 4
    // A varredura, a agregacao e a ordenacao: os candidatos de consulta.
    println!("=== 4. Varredura, agregacao e ordenacao (o `WHERE`, o `SUM`, o `ORDER BY`) ===\n");

    let m = TabelaMemoria::carregar(&mut t, &[], 0)?;
    let bytes_em_ram = m.bytes() as u64;
    println!(
        "  a tabela em memoria ocupa {:.1} MiB para {n} linhas\n",
        bytes_em_ram as f64 / 1_048_576.0
    );

    // Filtro sem mapa de igualdade: forca a varredura inteira. E o `WHERE`
    // que nao tem indice, que e o caso que uma GPU teoricamente atacaria.
    let varredura = Consulta {
        onde: vec![Filtro {
            coluna: 3,
            op: Operador::Maior,
            valor: Value::Int(99_000),
        }],
        ..Default::default()
    };
    let s_varrer = melhor_de(5, || {
        let inicio = Instant::now();
        let _ = m.selecionar(&varredura).unwrap();
        inicio.elapsed().as_secs_f64()
    });
    let mibs_varrer = mibs(bytes_em_ram, s_varrer);
    println!(
        "  varrer + filtrar {n} linhas ......... {:>7.1} ms  {:>8.0} MiB/s  ({:.1} M linhas/s)",
        s_varrer * 1000.0,
        mibs_varrer,
        n as f64 / s_varrer / 1e6
    );

    // Ordenar: o unico dos tres com custo super-linear, e por isso o melhor
    // candidato teorico do grupo.
    //
    // O CONTROLE importa aqui, e a primeira versao deste medidor nao o tinha:
    // a varredura filtrada devolve ~10 mil linhas e a ordenada devolve um
    // milhao. Comparar as duas mediria a materializacao, e nao a ordenacao --
    // e «bancada compara trabalho igual» vale para o medidor tambem. O
    // controle e a mesma consulta sem `ordenar`, devolvendo as mesmas
    // 1.000.000 de linhas.
    //
    // E as duas se medem INTERCALADAS, nao uma depois da outra. Medidas em
    // blocos, um pico de carga de outro processo cai inteiro num dos lados e a
    // subtracao chega a dar negativa -- foi o que aconteceu aqui: numa corrida
    // o controle deu 704 ms e a ordenada 649, ou seja «ordenar custa menos que
    // nao ordenar». Intercalado, o pico cai nos dois lados do par. E a mesma
    // correcao que o medidor do Profiler ja tinha exigido (DESEMPENHO §2.3.1).
    let sem_ordem = Consulta::default();
    let ordenada = Consulta {
        ordenar: vec![Ordem {
            coluna: 3,
            desc: false,
        }],
        ..Default::default()
    };
    let mut s_materializar = f64::MAX;
    let mut s_ordenar = f64::MAX;
    for volta in 0..6 {
        // A ordem dentro do par se inverte a cada volta, para nenhum dos dois
        // ficar sempre com a cache aquecida pelo outro.
        let primeiro_e_o_controle = volta % 2 == 0;
        let roda = |ordenar: bool| {
            let c = if ordenar { &ordenada } else { &sem_ordem };
            let inicio = Instant::now();
            let _ = m.selecionar(c).unwrap();
            inicio.elapsed().as_secs_f64()
        };
        if primeiro_e_o_controle {
            s_materializar = s_materializar.min(roda(false));
            s_ordenar = s_ordenar.min(roda(true));
        } else {
            s_ordenar = s_ordenar.min(roda(true));
            s_materializar = s_materializar.min(roda(false));
        }
    }
    let s_so_ordem = (s_ordenar - s_materializar).max(0.0);
    println!(
        "  varrer {n} linhas SEM ordem (controle) . {:>7.1} ms  {:>8.0} MiB/s",
        s_materializar * 1000.0,
        mibs(bytes_em_ram, s_materializar)
    );
    println!(
        "  varrer + ORDENAR {n} linhas ......... {:>7.1} ms  {:>8.0} MiB/s",
        s_ordenar * 1000.0,
        mibs(bytes_em_ram, s_ordenar)
    );
    println!(
        "  -> so a ordenacao, por subtracao ..... {:>7.1} ms ({:.0}% do ORDER BY)",
        s_so_ordem * 1000.0,
        100.0 * s_so_ordem / s_ordenar
    );

    // A agregacao (o `SUM`): uma soma por linha, sem dependencia entre elas.
    // No papel e o caso de livro para uma GPU -- por isso vale medir quanto
    // dela e conta e quanto e so passar os bytes.
    let coluna: Vec<i64> = (0..n).map(|i| i % 100_000).collect();
    let bytes_coluna = (coluna.len() * 8) as u64;
    let s_soma = melhor_de(5, || {
        let (s, _, _) = medir(0.2, |_| {
            let mut acc = 0i64;
            for v in std::hint::black_box(&coluna) {
                acc = acc.wrapping_add(*v);
            }
            acc as u64
        });
        s
    });
    println!(
        "  somar {n} valores i64 (o SUM puro) ... {:>7.1} ms  {:>8.0} MiB/s",
        s_soma * 1000.0,
        mibs(bytes_coluna, s_soma)
    );

    // A ordenacao pura de chaves, sem materializar linha: e o nucleo que uma
    // GPU ordenaria de verdade (radix sort), separado do resto.
    let mut chaves: Vec<u64> = (0..n as u64)
        .map(|i| (i * 2_654_435_761) % 1_000_000)
        .collect();
    let bytes_chaves = (chaves.len() * 8) as u64;
    let s_sort = melhor_de(3, || {
        let mut c = chaves.clone();
        let inicio = Instant::now();
        c.sort_unstable();
        let s = inicio.elapsed().as_secs_f64();
        chaves[0] = c[0];
        s
    });
    println!(
        "  ordenar {n} chaves u64, so o nucleo ... {:>7.1} ms  {:>8.0} MiB/s",
        s_sort * 1000.0,
        mibs(bytes_chaves, s_sort)
    );

    candidatos.push(Candidato {
        nome: "varredura com filtro (o WHERE sem indice)",
        mibs_cpu: mibs_varrer,
        peso: "a consulta inteira".into(),
    });
    candidatos.push(Candidato {
        nome: "agregacao SUM sobre uma coluna i64",
        mibs_cpu: mibs(bytes_coluna, s_soma),
        peso: "o nucleo do SUM, sem a leitura da linha".into(),
    });
    candidatos.push(Candidato {
        nome: "ordenacao de chaves u64 (o nucleo do ORDER BY)",
        mibs_cpu: mibs(bytes_chaves, s_sort),
        peso: format!("{:.0}% do ORDER BY", 100.0 * s_sort / s_ordenar.max(s_sort)),
    });

    // ---------------------------------------------------------------- 4b
    // O backup: e onde ha mais aritmetica uniforme contigua neste motor, entao
    // e o melhor candidato do repositorio. E por isso ele se mede INTEIRO, e
    // nao so na parcela que interessa a tese -- a pergunta nao e «quanto custa
    // o SHA-256», e sim «quanto do backup ele e».
    println!("\n=== 4b. O backup inteiro, e quanto dele e SHA-256 ===\n");

    let bytes_tabela = tamanho_da_pasta(&dir);
    let destino = std::env::temp_dir().join(format!("phx-gpu-bkp-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&destino);
    std::fs::create_dir_all(&destino)?;

    let inicio = Instant::now();
    let (zip, _rel) = phxsql_store::backup::executar_zip(&dir, &destino, "", "medidor", 0)?;
    let s_backup = inicio.elapsed().as_secs_f64();
    let bytes_zip = std::fs::metadata(&zip).map(|m| m.len()).unwrap_or(0);

    let s_sha = bytes_tabela as f64 / (sha_mib * 1_048_576.0);
    println!(
        "  a tabela em disco ocupa {:.1} MiB, e o zip saiu com {:.1} MiB ({:.2}x)",
        bytes_tabela as f64 / 1_048_576.0,
        bytes_zip as f64 / 1_048_576.0,
        bytes_tabela as f64 / bytes_zip.max(1) as f64
    );
    println!("  o backup inteiro custou ......... {:>7.2} s", s_backup);
    println!(
        "  o SHA-256 dentro dele ........... {:>7.2} s  ({:.1}% do backup, a {sha_mib:.0} MiB/s)",
        s_sha,
        100.0 * s_sha / s_backup
    );
    println!(
        "  -> com o SHA-256 de graca, o backup cairia para {:.2} s ({:.2}x)\n",
        s_backup - s_sha,
        s_backup / (s_backup - s_sha).max(1e-9)
    );

    candidatos.push(Candidato {
        nome: "SHA-256 do backup",
        mibs_cpu: sha_mib,
        peso: format!("{:.1}% do backup inteiro", 100.0 * s_sha / s_backup),
    });
    candidatos.push(Candidato {
        nome: "ChaCha20-Poly1305 de muitos slots",
        mibs_cpu: cifra_mib,
        peso: "so quando a tabela e cifrada".into(),
    });

    // ---------------------------------------------------------------- 5
    // A conta da travessia. Esta e a secao que decide.
    println!("\n=== 5. A conta da travessia: quando o barramento se paga ===\n");
    println!(
        "  Regra dada de presente para a GPU: nucleo de custo ZERO, volta de custo\n  \
         ZERO, barramento no PICO TEORICO. So a ida conta. Se ela perde assim,\n  \
         perde de qualquer jeito.\n"
    );
    println!(
        "  A GPU so pode ganhar se o barramento entregar os bytes MAIS RAPIDO do\n  \
         que a CPU ja os processa. Se a CPU e mais rapida que o barramento, o\n  \
         tamanho do dado nao importa: nao ha limiar, ela perde sempre.\n"
    );

    println!("  Quanto de operacao real cada candidato representa:\n");
    for c in &candidatos {
        println!("      {:<46} {}", c.nome, c.peso);
    }
    println!();

    for (nome_bus, pico) in BARRAMENTOS {
        println!("  --- {nome_bus}: {pico:.0} MiB/s de pico teorico (especificacao, nao medida)");
        println!(
            "      {:<46} {:>9}  {:>20}",
            "nucleo", "MiB/s CPU", "limiar"
        );
        for c in &candidatos {
            let veredito = if c.mibs_cpu >= pico {
                format!("NUNCA ({:.2}x o bus)", c.mibs_cpu / pico)
            } else {
                // B*(1/T - 1/P) > L  =>  B > L / (1/T - 1/P)
                let inv = 1.0 / c.mibs_cpu - 1.0 / pico;
                let bytes_mib = (LATENCIA_LANCAMENTO_US / 1e6) / inv;
                if bytes_mib > 1024.0 {
                    format!("> {:.1} GiB", bytes_mib / 1024.0)
                } else {
                    format!("> {bytes_mib:.2} MiB")
                }
            };
            println!("      {:<46} {:>9.0}  {veredito:>20}", c.nome, c.mibs_cpu);
        }
        println!();
    }

    println!(
        "  O limiar acima e o tamanho MINIMO de um lote para a ida ao barramento\n  \
         se pagar contra {LATENCIA_LANCAMENTO_US:.0} us de lancamento. Onde diz NUNCA, a CPU ja\n  \
         processa mais depressa do que o PCIe entrega, e nenhum tamanho conserta.\n"
    );

    println!("=== 6. A alternativa que nao fere a regra da casa: os 4 nucleos ===\n");
    println!(
        "  A `std` tem thread, e `paralelo.rs` ja tem o pedaco de rayon que este\n  \
         projeto usa. Aqui a pergunta e quanto cada nucleo GANHA dividido em {} --\n  \
         medido, e nao suposto, porque quem e preso a banda de memoria nao divide.\n",
        nucleos()
    );

    let blocos: Vec<Vec<u8>> = (0..16)
        .map(|k| (0..1024 * 1024).map(|i| ((i + k) % 251) as u8).collect())
        .collect();

    println!(
        "  {:<40} {:>10} {:>10} {:>8}",
        "nucleo, sobre 16 blocos de 1 MiB", "1 thread", "4 threads", "ganho"
    );
    println!("  {}", "-".repeat(72));

    for (nome, f) in [
        ("SHA-256", 0usize),
        ("ChaCha20-Poly1305 selar", 1),
        ("CRC-32", 2),
    ] {
        let trabalho = |b: &[u8]| -> u64 {
            match f {
                0 => sha256(b)[0] as u64,
                1 => cifra::selar(&chave, &nonce, b"", b).1[0] as u64,
                _ => crc32(b) as u64,
            }
        };

        let s_seq = melhor_de(3, || {
            let inicio = Instant::now();
            let mut acc = 0u64;
            for b in &blocos {
                acc = acc.wrapping_add(trabalho(std::hint::black_box(b)));
            }
            std::hint::black_box(acc);
            inicio.elapsed().as_secs_f64()
        });

        let s_par = melhor_de(3, || {
            let inicio = Instant::now();
            let mut acc = 0u64;
            std::thread::scope(|escopo| {
                let mut maos = Vec::new();
                for pedaco in blocos.chunks(blocos.len().div_ceil(nucleos())) {
                    maos.push(escopo.spawn(move || {
                        let mut meu = 0u64;
                        for b in pedaco {
                            meu = meu.wrapping_add(trabalho(std::hint::black_box(b)));
                        }
                        meu
                    }));
                }
                for mao in maos {
                    acc = acc.wrapping_add(mao.join().unwrap());
                }
            });
            std::hint::black_box(acc);
            inicio.elapsed().as_secs_f64()
        });

        println!(
            "  {nome:<40} {:>7.1} ms {:>7.1} ms {:>7.2}x",
            s_seq * 1000.0,
            s_par * 1000.0,
            s_seq / s_par
        );
    }

    // O DEFLATE do backup. A primeira versao desta medida usou um bloco
    // sintetico -- `(i+k) % 251`, que se comprime sozinho -- e deu 339 MiB/s,
    // uma conta que deixava 7,8 s dos 9,57 sem dono. Dado de mentira mede
    // mentira: a amostra passou a ser um pedaco do `.reg` DE VERDADE, que e o
    // que o backup comprime.
    let amostra = amostra_do_reg(&dir, 4 * 1024 * 1024);
    assert!(
        !amostra.is_empty(),
        "nao achei nenhum .reg para amostrar: sem ele esta medida nao existe"
    );
    let s_deflate = melhor_de(3, || {
        let inicio = Instant::now();
        let z = phxsql_core::zip::deflate(std::hint::black_box(&amostra));
        std::hint::black_box(z.len());
        inicio.elapsed().as_secs_f64()
    });
    let mibs_deflate = mibs(amostra.len() as u64, s_deflate);
    let s_deflate_tabela = (bytes_tabela as f64 / 1_048_576.0) / mibs_deflate;
    let resto = s_backup - s_sha - s_deflate_tabela;

    println!(
        "\n  E o resto do backup, medido em vez de suposto ({:.1} MiB de `.reg` real):\n",
        amostra.len() as f64 / 1_048_576.0
    );
    println!(
        "  {:<44} {:>9}  {:>7}",
        "parcela do backup", "segundos", "% dele"
    );
    println!("  {}", "-".repeat(64));
    for (nome, s) in [
        ("DEFLATE dos 236 MiB, a essa vazao", s_deflate_tabela),
        ("SHA-256 do manifesto", s_sha),
        ("o resto (ler, montar o zip, gravar)", resto),
    ] {
        println!("  {nome:<44} {s:>9.2}  {:>6.1}%", 100.0 * s / s_backup);
    }
    println!("  {:<44} {s_backup:>9.2}  {:>6.1}%", "TOTAL medido", 100.0);
    println!(
        "\n  DEFLATE do `.reg` real: {mibs_deflate:.0} MiB/s -- e ele e busca de repeticao\n  \
         num dicionario que depende do byte anterior: o oposto do que a GPU acelera.\n"
    );

    println!(
        "RESULTADO {}",
        json_do_resultado(&Resumo {
            copia,
            leitura,
            crc_pagina,
            crc_mib,
            sha_mib,
            cifra_pagina,
            cifra_mib,
            us_por_linha,
            pct_crc,
            us_busca: s_buscar * 1e6 / buscas as f64,
            ms_varrer: s_varrer * 1000.0,
            mibs_varrer,
            ms_ordenar: s_ordenar * 1000.0,
            ms_so_ordem: s_so_ordem * 1000.0,
            ms_materializar: s_materializar * 1000.0,
            s_backup,
            s_sha,
            ms_sort: s_sort * 1000.0,
            mibs_sort: mibs(bytes_chaves, s_sort),
            mib_em_ram: bytes_em_ram as f64 / 1_048_576.0,
            mib_em_disco: bytes_tabela as f64 / 1_048_576.0,
        })
    );

    let _ = std::fs::remove_dir_all(&dir);
    Ok(())
}

/// Os numeros da corrida, para a ultima linha em JSON -- como na `carga`.
struct Resumo {
    copia: f64,
    leitura: f64,
    crc_pagina: f64,
    crc_mib: f64,
    sha_mib: f64,
    cifra_pagina: f64,
    cifra_mib: f64,
    us_por_linha: f64,
    pct_crc: f64,
    us_busca: f64,
    ms_varrer: f64,
    mibs_varrer: f64,
    ms_ordenar: f64,
    ms_so_ordem: f64,
    ms_materializar: f64,
    s_backup: f64,
    s_sha: f64,
    ms_sort: f64,
    mibs_sort: f64,
    mib_em_ram: f64,
    mib_em_disco: f64,
}

fn json_do_resultado(r: &Resumo) -> String {
    format!(
        "{{\"banda\":{{\"copia_mibs\":{:.0},\"leitura_mibs\":{:.0}}},\
         \"nucleos\":{{\"crc_4k_mibs\":{:.0},\"crc_1m_mibs\":{:.0},\"sha256_mibs\":{:.0},\
         \"cifra_4k_mibs\":{:.0},\"cifra_1m_mibs\":{:.0}}},\
         \"insercao\":{{\"us_por_linha\":{:.2},\"pct_crc\":{:.2}}},\
         \"busca_us\":{:.2},\
         \"consulta\":{{\"varrer_ms\":{:.1},\"varrer_mibs\":{:.0},\"materializar_ms\":{:.1},\
         \"ordenar_ms\":{:.1},\"so_ordem_ms\":{:.1},\"sort_ms\":{:.1},\"sort_mibs\":{:.0}}},\
         \"backup\":{{\"inteiro_s\":{:.2},\"sha256_s\":{:.2}}},\
         \"tabela\":{{\"ram_mib\":{:.1},\"disco_mib\":{:.1}}}}}",
        r.copia,
        r.leitura,
        r.crc_pagina,
        r.crc_mib,
        r.sha_mib,
        r.cifra_pagina,
        r.cifra_mib,
        r.us_por_linha,
        r.pct_crc,
        r.us_busca,
        r.ms_varrer,
        r.mibs_varrer,
        r.ms_materializar,
        r.ms_ordenar,
        r.ms_so_ordem,
        r.ms_sort,
        r.mibs_sort,
        r.s_backup,
        r.s_sha,
        r.mib_em_ram,
        r.mib_em_disco,
    )
}

/// Um pedaco do `.reg` de verdade, para medir compressao sobre o dado que o
/// backup realmente comprime.
///
/// Existe porque a versao anterior media um bloco sintetico que se comprimia
/// sozinho, e a conta do backup nao fechava por 7,8 s.
fn amostra_do_reg(dir: &std::path::Path, teto: usize) -> Vec<u8> {
    fn achar(dir: &std::path::Path) -> Option<std::path::PathBuf> {
        let entradas = std::fs::read_dir(dir).ok()?;
        let mut pastas = Vec::new();
        for e in entradas.flatten() {
            let caminho = e.path();
            if caminho.is_dir() {
                pastas.push(caminho);
            } else if caminho.extension().is_some_and(|x| x == "reg") {
                return Some(caminho);
            }
        }
        pastas.into_iter().find_map(|p| achar(&p))
    }
    match achar(dir).and_then(|c| std::fs::read(c).ok()) {
        Some(mut bytes) => {
            bytes.truncate(teto);
            bytes
        }
        // Sem `.reg` a medida nao existe -- e devolver um bloco sintetico aqui
        // seria repor exatamente o furo que esta funcao veio tapar.
        None => Vec::new(),
    }
}

/// Quantos bytes os arquivos da tabela ocupam. O backup passa todos eles pelo
/// SHA-256, entao e este o volume que um nucleo de hash veria.
fn tamanho_da_pasta(dir: &std::path::Path) -> u64 {
    let mut total = 0u64;
    if let Ok(entradas) = std::fs::read_dir(dir) {
        for e in entradas.flatten() {
            match e.metadata() {
                Ok(m) if m.is_dir() => total += tamanho_da_pasta(&e.path()),
                Ok(m) => total += m.len(),
                Err(_) => {}
            }
        }
    }
    total
}
