//! Frente H (colmeia/hive), itens H1+H2 -- `docs/propostas/colmeia.md` §1 e
//! `docs/propostas/colmeia-estrutura.md`: protótipo mínimo do formato PSHV
//! (inspirado no REGF do Registro do Windows) e a medição da premissa que
//! decide se ele vale a pena construir de verdade.
//!
//! ```bash
//! cargo build --release --examples -p phxsql-store   # binario novo -- pétrea
//! target/release/examples/custo-da-colmeia            # sweep completo
//! target/release/examples/custo-da-colmeia 10000      # 1 tamanho (calibragem)
//! ```
//!
//! # O que isto NÃO é
//!
//! Isto é PROTÓTIPO DE BANCADA, não o motor real: sem `ColumnType::Hive`, sem
//! `TipoDatabase::Hive` despachando nada, sem aval de formato do dono. O
//! `TipoDatabase::Hive` continua `motor_pronto()==false`. O arquivo `.hivep`
//! que este medidor grava em `std::env::temp_dir()` é descartável -- nasce e
//! morre dentro da própria corrida, do mesmo jeito que as tabelas Padrão de
//! `onde-doi.rs`/`cache-frio.rs`.
//!
//! # H1 -- o protótipo PSHV mínimo
//!
//! Uma colmeia é um arquivo append-only (pétrea: "a ordem de digitação é
//! sagrada, nunca reaproveita slot" -- aqui não há UPDATE nem DELETE, só
//! construção e leitura, então o append-only é automático) com:
//!
//! - cabeçalho de 128 bytes (assinatura `PHXHIV\0\0`, versão, offset da célula
//!   RAIZ, CRC-32 do próprio cabeçalho -- convenção da casa, como `.reg`/`.ndx`);
//! - células **NÓ** (nome + lista de subchaves ordenada por hash + lista de
//!   valores ordenada por hash) e células **VALOR** (nome + bytes), exatamente
//!   o desenho de `colmeia-estrutura.md` §3-4 (`nk`/`lf`/`vk` do REGF, com o
//!   **nosso** CRC-32 como hash e sem reúso de célula).
//!
//! Ler um caminho (`"g3/s5/v2"`) desce a árvore: bloco base -> NÓ raiz -> busca
//! binária na lista de subchaves por `hash("g3")` -> NÓ `g3` -> busca binária
//! por `hash("s5")` -> NÓ `s5` -> busca binária na lista de VALORES por
//! `hash("v2")` -> célula VALOR. É o caminho quente descrito em
//! `colmeia-estrutura.md` §3, e é exatamente o que este medidor cronometra.
//!
//! Simplificação declarada (protótipo, não motor): sem bins de 4 KiB nem
//! página cacheada por página -- o arquivo inteiro (pequeno, config-shaped)
//! é lido para memória uma vez em `Colmeia::abrir`, e a descida da árvore
//! acontece sobre esse buffer. É o platô de um cache já quente (o que
//! `colmeia-estrutura.md` §4 chama de "mapa de leitura é o nosso cache"), não
//! o custo de aquecê-lo -- esse fica fora do escopo de H1/H2.
//!
//! # H2 -- a premissa: a colmeia lê config-shaped mais rápido que o Padrão?
//!
//! O Padrão é o `phxsql_store::table::Table` de verdade: uma tabela com
//! `chave` (índice único) e `valor`, o MESMO par de dados da colmeia. A leitura
//! do Padrão é a mesma "busca de ponto" que um `WHERE chave = ?` faz no motor
//! vivo: `Table::buscar(indice, [chave])` (desce o `.ndx`) seguido de
//! `Table::ler(rowid)` (lê o `.reg`) -- não uma varredura.
//!
//! **O trabalho comparado, numa frase:** ler o MESMO par chave->valor, pelos
//! MESMOS N pontos config-shaped, por busca de ponto -> busca de ponto nos
//! dois lados -- descida de árvore de células na colmeia, índice único +
//! leitura de registro no Padrão -- nunca "varre tudo" de um lado contra
//! "busca 1" do outro (a regra 3/4 de `bancada/LEIA-ME.md`, já errada aqui
//! duas vezes: 41x e 5x).
//!
//! Ambos os lados são medidos QUENTES, e isso é declarado, não escondido: a
//! colmeia porque o arquivo inteiro cabe no buffer carregado no `abrir`; o
//! Padrão porque o cache de páginas do `.ndx` é dimensionado (`definir_cache_paginas`)
//! para caber o índice inteiro (dado config é pequeno) e uma passada de
//! aquecimento roda ANTES do cronômetro, descartada da medição.

use std::io::Write;
use std::time::Instant;

use phxsql_core::crc::crc32;
use phxsql_core::schema::{Column, IndexColumn, IndexDef, Schema};
use phxsql_core::types::ColumnType;
use phxsql_core::value::Value;
use phxsql_store::ndx::definir_cache_paginas;
use phxsql_store::table::Table;

// ======================================================================
// splitmix64 -- gerador determinístico (regra 1 da bancada: "mesmos dados,
// sem sorteio"). Mesma receita de `custo-do-vizinho.rs`/`phxsql_core::uuid`.
// ======================================================================

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
    fn entre(&mut self, limite: usize) -> usize {
        (self.proximo() % limite as u64) as usize
    }
}

// ======================================================================
// H1: o formato PSHV mínimo -- construção
// ======================================================================

const ASSINATURA: &[u8; 8] = b"PHXHIV\0\0";
const TAM_CABECALHO: usize = 128;
const TIPO_NO: u8 = 1;
const TIPO_VALOR: u8 = 2;
const STATUS_ATIVA: u8 = 1;

/// Um nó em construção, antes de virar bytes. `BTreeMap` só para a ordem de
/// construção ser determinística -- a busca em disco usa a lista ordenada por
/// HASH, nunca por nome (a árvore em si não depende desta ordem).
struct NoBuilder {
    subchaves: std::collections::BTreeMap<String, NoBuilder>,
    valores: std::collections::BTreeMap<String, Vec<u8>>,
}

impl NoBuilder {
    fn nova() -> Self {
        NoBuilder {
            subchaves: Default::default(),
            valores: Default::default(),
        }
    }

    /// Desce/cria a cadeia de subchaves indicada por `segmentos` e grava o
    /// valor `dado` sob o nome `nome_valor` na chave final -- o `hive_gravar`
    /// do esboço de protocolo (`colmeia-estrutura.md` §5), em memória.
    fn gravar(&mut self, segmentos: &[&str], nome_valor: &str, dado: Vec<u8>) {
        match segmentos.split_first() {
            None => {
                self.valores.insert(nome_valor.to_string(), dado);
            }
            Some((cabeca, resto)) => {
                self.subchaves
                    .entry(cabeca.to_string())
                    .or_insert_with(NoBuilder::nova)
                    .gravar(resto, nome_valor, dado);
            }
        }
    }
}

fn u32_le(v: u32, buf: &mut Vec<u8>) {
    buf.extend_from_slice(&v.to_le_bytes());
}
fn u64_le(v: u64, buf: &mut Vec<u8>) {
    buf.extend_from_slice(&v.to_le_bytes());
}

/// Serializa uma célula VALOR (nome + dado) no fim de `celulas` -- NUNCA no
/// meio (append-only, a pétrea da ordem de digitação por outro nome: aqui não
/// há reescrita, só nascimento). Devolve o offset ABSOLUTO no arquivo do byte
/// de STATUS da célula (já somado o cabeçalho de 128 bytes) -- não do campo
/// `tam` que vem 4 bytes antes: `tam` só serve para varredura sequencial (uma
/// futura compactação), a leitura por ponto nunca precisa dele, e por isso as
/// listas de subchaves/valores apontam direto para o status.
fn escrever_valor(celulas: &mut Vec<u8>, nome: &str, dado: &[u8]) -> u64 {
    let offset = (TAM_CABECALHO + celulas.len() + 4) as u64;
    let nome_b = nome.as_bytes();
    let mut corpo = Vec::with_capacity(2 + nome_b.len() + 4 + dado.len() + 2);
    corpo.push(TIPO_VALOR);
    corpo.extend_from_slice(&(nome_b.len() as u16).to_le_bytes());
    corpo.extend_from_slice(nome_b);
    corpo.extend_from_slice(&(dado.len() as u32).to_le_bytes());
    corpo.extend_from_slice(dado);
    let tam = (4 + 1 + corpo.len()) as u32; // tam + status + corpo
    u32_le(tam, celulas);
    celulas.push(STATUS_ATIVA);
    celulas.extend_from_slice(&corpo);
    offset
}

/// Serializa um NÓ em pós-ordem: as subchaves e os valores nascem ANTES do
/// nó-pai (para o pai já saber o offset delas ao gravar suas listas), exatamente
/// como uma árvore de células endereçadas por offset exige. Devolve o offset
/// absoluto do próprio NÓ.
fn escrever_no(celulas: &mut Vec<u8>, nome: &str, no: &NoBuilder) -> u64 {
    let mut subchaves_entradas: Vec<(u32, u64)> = no
        .subchaves
        .iter()
        .map(|(nome_filho, filho)| {
            let off = escrever_no(celulas, nome_filho, filho);
            (crc32(nome_filho.as_bytes()), off)
        })
        .collect();
    let mut valores_entradas: Vec<(u32, u64)> = no
        .valores
        .iter()
        .map(|(nome_valor, dado)| {
            let off = escrever_valor(celulas, nome_valor, dado);
            (crc32(nome_valor.as_bytes()), off)
        })
        .collect();
    // Ordenadas por HASH -- é o que permite busca BINÁRIA na leitura (`lf`/`lh`
    // do REGF, `colmeia-estrutura.md` §3).
    subchaves_entradas.sort_by_key(|&(h, _)| h);
    valores_entradas.sort_by_key(|&(h, _)| h);

    // Offset do byte de STATUS -- mesma convenção de `escrever_valor` acima.
    let offset = (TAM_CABECALHO + celulas.len() + 4) as u64;
    let nome_b = nome.as_bytes();
    let mut corpo = Vec::new();
    corpo.push(TIPO_NO);
    corpo.extend_from_slice(&(nome_b.len() as u16).to_le_bytes());
    corpo.extend_from_slice(nome_b);
    corpo.extend_from_slice(&(subchaves_entradas.len() as u32).to_le_bytes());
    for (h, o) in &subchaves_entradas {
        u32_le(*h, &mut corpo);
        u64_le(*o, &mut corpo);
    }
    corpo.extend_from_slice(&(valores_entradas.len() as u32).to_le_bytes());
    for (h, o) in &valores_entradas {
        u32_le(*h, &mut corpo);
        u64_le(*o, &mut corpo);
    }
    let tam = (4 + 1 + corpo.len()) as u32;
    u32_le(tam, celulas);
    celulas.push(STATUS_ATIVA);
    celulas.extend_from_slice(&corpo);
    offset
}

/// Grava uma colmeia PSHV completa a partir da árvore em memória, num único
/// arquivo (cabeçalho + células), com `fsync` -- a mesma disciplina de
/// durabilidade que o resto da casa aplica a um arquivo novo.
fn gravar_colmeia(caminho: &std::path::Path, raiz: &NoBuilder) -> std::io::Result<()> {
    let mut celulas = Vec::new();
    let raiz_offset = escrever_no(&mut celulas, "", raiz);

    let mut cabecalho = vec![0u8; TAM_CABECALHO];
    cabecalho[0..8].copy_from_slice(ASSINATURA);
    cabecalho[8..12].copy_from_slice(&1u32.to_le_bytes()); // versao
    cabecalho[12..20].copy_from_slice(&raiz_offset.to_le_bytes());
    let crc = crc32(&cabecalho[0..TAM_CABECALHO - 4]);
    cabecalho[TAM_CABECALHO - 4..].copy_from_slice(&crc.to_le_bytes());

    let mut arquivo = std::fs::File::create(caminho)?;
    arquivo.write_all(&cabecalho)?;
    arquivo.write_all(&celulas)?;
    arquivo.sync_all()?;
    Ok(())
}

// ======================================================================
// H1: o formato PSHV mínimo -- leitura (o caminho quente)
// ======================================================================

/// Uma colmeia aberta para leitura: o arquivo inteiro em memória (a
/// simplificação declarada no topo do arquivo -- "nosso cache", já quente).
struct Colmeia {
    buf: Vec<u8>,
    raiz_offset: u64,
}

fn le_u32(buf: &[u8], o: usize) -> u32 {
    u32::from_le_bytes(buf[o..o + 4].try_into().unwrap())
}
fn le_u64(buf: &[u8], o: usize) -> u64 {
    u64::from_le_bytes(buf[o..o + 8].try_into().unwrap())
}
fn le_u16(buf: &[u8], o: usize) -> u16 {
    u16::from_le_bytes(buf[o..o + 2].try_into().unwrap())
}

impl Colmeia {
    fn abrir(caminho: &std::path::Path) -> std::io::Result<Colmeia> {
        let buf = std::fs::read(caminho)?;
        assert_eq!(&buf[0..8], ASSINATURA, "assinatura PSHV invalida");
        let crc_gravado = le_u32(&buf, TAM_CABECALHO - 4);
        let crc_calc = crc32(&buf[0..TAM_CABECALHO - 4]);
        assert_eq!(
            crc_gravado, crc_calc,
            "CRC-32 do cabecalho da colmeia nao bate"
        );
        let raiz_offset = le_u64(&buf, 12);
        Ok(Colmeia { buf, raiz_offset })
    }

    /// Nome gravado numa célula NÓ ou VALOR (ambas comecam status+tipo+len+nome).
    fn nome_da_celula(&self, offset: u64) -> &str {
        let o = offset as usize;
        // o+4 = tam (u32) já consumido pelo chamador via `offset` apontar pro
        // status; aqui offset aponta para o BYTE de status.
        let nome_len = le_u16(&self.buf, o + 2) as usize;
        std::str::from_utf8(&self.buf[o + 4..o + 4 + nome_len]).unwrap()
    }

    /// Busca binária por `hash_alvo` numa lista de entradas (hash u32 + offset
    /// u64, 12 bytes cada) que começa em `o` e tem `n` elementos -- e confere o
    /// NOME da célula achada contra `nome_alvo` (defesa contra colisão de
    /// CRC-32; varre os vizinhos de mesmo hash se o primeiro achado não bater,
    /// o que nunca acontece neste conjunto sintético, mas a leitura não confia
    /// em "nunca" sem checar).
    fn achar(&self, o: usize, n: u32, hash_alvo: u32, nome_alvo: &str) -> Option<u64> {
        let n = n as usize;
        let mut lo = 0usize;
        let mut hi = n;
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let h = le_u32(&self.buf, o + mid * 12);
            match h.cmp(&hash_alvo) {
                std::cmp::Ordering::Less => lo = mid + 1,
                std::cmp::Ordering::Greater => hi = mid,
                std::cmp::Ordering::Equal => {
                    // achou o hash -- varre para os dois lados enquanto o hash
                    // continuar igual, conferindo o nome de verdade.
                    let mut i = mid;
                    loop {
                        let off = le_u64(&self.buf, o + i * 12 + 4);
                        if self.nome_da_celula(off) == nome_alvo {
                            return Some(off);
                        }
                        if i == 0 || le_u32(&self.buf, o + (i - 1) * 12) != hash_alvo {
                            break;
                        }
                        i -= 1;
                    }
                    let mut i = mid + 1;
                    while i < n && le_u32(&self.buf, o + i * 12) == hash_alvo {
                        let off = le_u64(&self.buf, o + i * 12 + 4);
                        if self.nome_da_celula(off) == nome_alvo {
                            return Some(off);
                        }
                        i += 1;
                    }
                    return None;
                }
            }
        }
        None
    }

    /// O caminho quente: bloco base -> NÓ raiz -> ... -> NÓ final -> VALOR,
    /// só por descida e busca binária, sem varrer nada. Devolve os bytes do
    /// valor (fatiados do buffer -- sem cópia, é o que o desenho ganha por não
    /// decodificar tipos relacionais no caminho).
    fn ler(&self, caminho: &str) -> Option<&[u8]> {
        let segmentos: Vec<&str> = caminho.split('/').filter(|s| !s.is_empty()).collect();
        let (ultimo, chaves) = segmentos.split_last()?;
        let mut offset = self.raiz_offset;
        for seg in chaves {
            let o = offset as usize;
            let nome_len = le_u16(&self.buf, o + 2) as usize;
            let mut p = o + 4 + nome_len;
            let n_sub = le_u32(&self.buf, p);
            p += 4;
            offset = self.achar(p, n_sub, crc32(seg.as_bytes()), seg)?;
        }
        // NÓ final: pula a lista de subchaves e busca na lista de VALORES.
        let o = offset as usize;
        let nome_len = le_u16(&self.buf, o + 2) as usize;
        let mut p = o + 4 + nome_len;
        let n_sub = le_u32(&self.buf, p);
        p += 4 + (n_sub as usize) * 12;
        let n_val = le_u32(&self.buf, p);
        p += 4;
        let valor_offset = self.achar(p, n_val, crc32(ultimo.as_bytes()), ultimo)?;

        let vo = valor_offset as usize;
        let nl = le_u16(&self.buf, vo + 2) as usize;
        let mut q = vo + 4 + nl;
        let dado_len = le_u32(&self.buf, q) as usize;
        q += 4;
        Some(&self.buf[q..q + dado_len])
    }
}

// ======================================================================
// O conjunto de dados config-shaped: G grupos -> S subchaves -> V valores
// por subchave-folha -- "poucos valores por nó", como o Registro
// (`colmeia.md` §1: "config e escrita raramente, lida o tempo todo").
// ======================================================================

const V_POR_FOLHA: usize = 8;
const S_POR_GRUPO: usize = 32;

struct Ponto {
    caminho: String, // "gN/sN/vN"
    valor: Vec<u8>,
}

fn gerar_pontos(n_alvo: usize, seed: u64) -> Vec<Ponto> {
    let mut rng = Splitmix64::novo(seed);
    let folhas = n_alvo.div_ceil(V_POR_FOLHA);
    let grupos = folhas.div_ceil(S_POR_GRUPO);
    let mut pontos = Vec::with_capacity(n_alvo);
    'fora: for g in 0..grupos {
        for s in 0..S_POR_GRUPO {
            for v in 0..V_POR_FOLHA {
                if pontos.len() >= n_alvo {
                    break 'fora;
                }
                let caminho = format!("g{g}/s{s}/v{v}");
                // Valor pequeno, tipico de config: 8-64 bytes, texto.
                let tam = 8 + rng.entre(56);
                let valor: Vec<u8> = (0..tam).map(|_| b'a' + (rng.entre(26) as u8)).collect();
                pontos.push(Ponto { caminho, valor });
            }
        }
    }
    pontos
}

// ======================================================================
// O Padrão: a MESMA tabela chave->valor, pelo motor de verdade
// (`phxsql_store::table::Table`), lida por busca de ponto no indice unico.
// ======================================================================

fn colunas_padrao() -> Vec<Column> {
    vec![
        Column::new("chave", ColumnType::Str(64)).obrigatoria(),
        Column::new("valor", ColumnType::Str(128)).obrigatoria(),
    ]
}

fn montar_padrao(dir: &std::path::Path, pontos: &[Ponto]) -> Table {
    let _ = std::fs::remove_dir_all(dir);
    std::fs::create_dir_all(dir).unwrap();
    let esquema = Schema::new(
        "config",
        colunas_padrao(),
        vec![IndexDef::new("porChave", vec![IndexColumn::asc(0)]).unico()],
    )
    .unwrap();
    let mut t = Table::criar(dir, esquema).unwrap();
    for p in pontos {
        let valor_txt = String::from_utf8(p.valor.clone()).unwrap();
        t.inserir(&[Value::Str(p.caminho.clone()), Value::Str(valor_txt)])
            .unwrap();
    }
    t.sincronizar().unwrap();
    t
}

/// A "busca de ponto" do Padrao: indice unico -> ler(rowid). E' o mesmo par
/// (WHERE chave = ?) que o servidor faz para uma consulta pontual.
fn ler_padrao(t: &mut Table, chave: &str) -> Option<Vec<u8>> {
    let achados = t
        .buscar("porChave", &[Value::Str(chave.to_string())])
        .ok()?;
    let rowid = *achados.first()?;
    let linha = t.ler(rowid).ok()??;
    match &linha[1] {
        Value::Str(s) => Some(s.clone().into_bytes()),
        _ => None,
    }
}

// ======================================================================
// H1 -- prova de read-back (gravou -> leu devolve os MESMOS bytes)
// ======================================================================

fn provar_read_back(pontos: &[Ponto], colmeia: &Colmeia, tabela: &mut Table) {
    for p in pontos {
        let lido_colmeia = colmeia
            .ler(&p.caminho)
            .unwrap_or_else(|| panic!("colmeia: caminho {} nao encontrado", p.caminho));
        assert_eq!(
            lido_colmeia,
            p.valor.as_slice(),
            "colmeia: read-back divergiu em {}",
            p.caminho
        );

        let lido_padrao = ler_padrao(tabela, &p.caminho)
            .unwrap_or_else(|| panic!("padrao: chave {} nao encontrada", p.caminho));
        assert_eq!(
            lido_padrao, p.valor,
            "padrao: read-back divergiu em {}",
            p.caminho
        );
    }
    println!(
        "  read-back conferido nos dois lados: {} pontos, gravou == leu (byte a byte).",
        pontos.len()
    );
}

// ======================================================================
// H2 -- a medicao: mediana + faixa min-max sobre REPETICOES, com aquecimento
// separado do cronometro.
// ======================================================================

fn mediana(mut v: Vec<f64>) -> f64 {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    v[v.len() / 2]
}

struct Medida {
    mediana_us: f64,
    faixa_us: (f64, f64),
}

fn medir<F: FnMut(&str) -> usize>(consultas: &[String], reps: usize, mut op: F) -> Medida {
    // Aquecimento: uma passada inteira, FORA do cronometro -- e' aqui que
    // qualquer cache (o buffer da colmeia ja nasce quente; o `.ndx` do Padrao
    // aquece aqui) esquenta, para o loop medido nao pagar a primeira leitura
    // fria de ninguem.
    let mut ancora = 0usize;
    for c in consultas {
        ancora ^= op(c);
    }
    std::hint::black_box(ancora);

    let mut amostras = Vec::with_capacity(reps);
    for _ in 0..reps {
        let inicio = Instant::now();
        let mut ancora = 0usize;
        for c in consultas {
            ancora ^= std::hint::black_box(op(c));
        }
        std::hint::black_box(ancora);
        let us_total = inicio.elapsed().as_secs_f64() * 1e6;
        amostras.push(us_total / consultas.len() as f64);
    }
    let min = amostras.iter().cloned().fold(f64::INFINITY, f64::min);
    let max = amostras.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    Medida {
        mediana_us: mediana(amostras),
        faixa_us: (min, max),
    }
}

fn arred(x: f64, casas: i32) -> f64 {
    let f = 10f64.powi(casas);
    (x * f).round() / f
}

fn maquina_info() -> (String, f64) {
    let nucleos = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(0);
    let carga1min = std::fs::read_to_string("/proc/loadavg")
        .ok()
        .and_then(|s| s.split_whitespace().next().map(str::to_string))
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(-1.0);
    (format!("container Linux, {nucleos} nucleos"), carga1min)
}

fn agora_utc_min() -> String {
    std::process::Command::new("date")
        .args(["-u", "+%Y-%m-%d %H:%M"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "desconhecido".to_string())
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let tamanhos: Vec<usize> = if let Some(n) = args.get(1).and_then(|s| s.parse().ok()) {
        vec![n]
    } else {
        vec![1_000, 10_000, 100_000]
    };

    println!("=== custo-da-colmeia: protótipo PSHV x Padrão, config-shaped (H1+H2) ===\n");
    println!("trabalho comparado: leitura de 1 par chave->valor por BUSCA DE PONTO");
    println!("(caminho completo -> bytes do valor) nos dois lados -- descida de");
    println!("árvore de células na colmeia, índice único + leitura de registro no");
    println!("Padrão. Nunca 'varre tudo' de um lado contra 'busca 1' do outro.\n");

    let (maquina, carga1min) = maquina_info();
    println!("máquina: {maquina}, carga(1min) no início = {carga1min}\n");

    // Cache do `.ndx` grande o bastante para caber o índice inteiro em
    // qualquer um dos tamanhos do sweep -- dado config e' pequeno, e a
    // premissa e' sobre leitura QUENTE dos dois lados, nao sobre quem tem
    // cache maior.
    definir_cache_paginas(1_000_000);

    const REPS: usize = 15;
    const CONSULTAS_POR_REP: usize = 4_000;
    let seed_dados = 0x5EED_C01B_EE1Au64;

    let mut resultados_json = vec![];

    for &n_alvo in &tamanhos {
        println!("--- N alvo = {n_alvo} pontos config-shaped ---");
        let pontos = gerar_pontos(n_alvo, 0x005E_ED12_34C0_FFEE_u64 ^ n_alvo as u64);
        let n_real = pontos.len();

        // --- H1: montar os dois lados com os MESMOS dados ---
        let mut raiz = NoBuilder::nova();
        for p in &pontos {
            let segmentos: Vec<&str> = p.caminho.split('/').collect();
            let (ultimo, chaves) = segmentos.split_last().unwrap();
            raiz.gravar(chaves, ultimo, p.valor.clone());
        }
        let arquivo_colmeia = std::env::temp_dir().join(format!(
            "phx-colmeia-{}-{}.hivep",
            std::process::id(),
            n_alvo
        ));
        gravar_colmeia(&arquivo_colmeia, &raiz).expect("gravar colmeia");
        let colmeia = Colmeia::abrir(&arquivo_colmeia).expect("abrir colmeia");

        let dir_padrao = std::env::temp_dir().join(format!(
            "phx-colmeia-padrao-{}-{}",
            std::process::id(),
            n_alvo
        ));
        let mut tabela = montar_padrao(&dir_padrao, &pontos);

        // --- H1: prova de read-back, gravou -> leu, nos dois lados ---
        provar_read_back(&pontos, &colmeia, &mut tabela);

        // --- H2: as consultas -- mesmos M caminhos, mesma ordem, nos dois lados ---
        let mut rng = Splitmix64::novo(seed_dados ^ n_alvo as u64);
        let consultas: Vec<String> = (0..CONSULTAS_POR_REP)
            .map(|_| pontos[rng.entre(n_real)].caminho.clone())
            .collect();

        let medida_colmeia = medir(&consultas, REPS, |c| colmeia.ler(c).map_or(0, |v| v.len()));
        let medida_padrao = medir(&consultas, REPS, |c| {
            ler_padrao(&mut tabela, c).map_or(0, |v| v.len())
        });

        let razao = medida_padrao.mediana_us / medida_colmeia.mediana_us;
        let faixas_se_cruzam = medida_colmeia.faixa_us.1 >= medida_padrao.faixa_us.0
            && medida_padrao.faixa_us.1 >= medida_colmeia.faixa_us.0;
        let veredito = if faixas_se_cruzam {
            "dentro_do_ruido"
        } else if medida_colmeia.mediana_us < medida_padrao.mediana_us {
            "colmeia_ganha"
        } else {
            "padrao_ganha"
        };

        println!(
            "  colmeia: mediana {:>7.3} us/op  faixa [{:.3}; {:.3}]",
            medida_colmeia.mediana_us, medida_colmeia.faixa_us.0, medida_colmeia.faixa_us.1
        );
        println!(
            "  padrao : mediana {:>7.3} us/op  faixa [{:.3}; {:.3}]",
            medida_padrao.mediana_us, medida_padrao.faixa_us.0, medida_padrao.faixa_us.1
        );
        println!(
            "  razao padrao/colmeia = {razao:.2}x   faixas se cruzam? {}   veredito: {veredito}\n",
            if faixas_se_cruzam { "sim" } else { "nao" }
        );

        resultados_json.push(phxsql_core::json::Json::objeto(vec![
            ("n_alvo", phxsql_core::json::Json::de_u64(n_alvo as u64)),
            ("n_real", phxsql_core::json::Json::de_u64(n_real as u64)),
            (
                "consultas_por_repeticao",
                phxsql_core::json::Json::de_u64(CONSULTAS_POR_REP as u64),
            ),
            ("repeticoes", phxsql_core::json::Json::de_u64(REPS as u64)),
            (
                "colmeia_mediana_us",
                phxsql_core::json::Json::Numero(arred(medida_colmeia.mediana_us, 4)),
            ),
            (
                "colmeia_faixa_us",
                phxsql_core::json::Json::Lista(vec![
                    phxsql_core::json::Json::Numero(arred(medida_colmeia.faixa_us.0, 4)),
                    phxsql_core::json::Json::Numero(arred(medida_colmeia.faixa_us.1, 4)),
                ]),
            ),
            (
                "padrao_mediana_us",
                phxsql_core::json::Json::Numero(arred(medida_padrao.mediana_us, 4)),
            ),
            (
                "padrao_faixa_us",
                phxsql_core::json::Json::Lista(vec![
                    phxsql_core::json::Json::Numero(arred(medida_padrao.faixa_us.0, 4)),
                    phxsql_core::json::Json::Numero(arred(medida_padrao.faixa_us.1, 4)),
                ]),
            ),
            (
                "razao_padrao_sobre_colmeia",
                phxsql_core::json::Json::Numero(arred(razao, 4)),
            ),
            (
                "faixas_se_cruzam",
                phxsql_core::json::Json::de_bool(faixas_se_cruzam),
            ),
            ("veredito", phxsql_core::json::Json::texto_de(veredito)),
        ]));

        drop(tabela);
        let _ = std::fs::remove_dir_all(&dir_padrao);
        let _ = std::fs::remove_file(&arquivo_colmeia);
    }

    // ------------------------------------------------------------- grava
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../bancada/colmeia");
    std::fs::create_dir_all(&dir).ok();
    let alvo = dir.join("resultados.json");

    let raiz_json = phxsql_core::json::Json::objeto(vec![
        (
            "bancada",
            phxsql_core::json::Json::texto_de("custo-da-colmeia"),
        ),
        (
            "o_que_mede",
            phxsql_core::json::Json::texto_de(
                "H2: leitura por busca de ponto (caminho->valor) config-shaped, \
                 protótipo PSHV (H1) contra o motor Padrão de verdade, mesma máquina",
            ),
        ),
        (
            "trabalho_comparado",
            phxsql_core::json::Json::texto_de(
                "mesmo par chave->valor, busca de ponto -> busca de ponto nos dois \
                 lados (descida de arvore de celulas x indice unico+leitura de \
                 registro); nunca varredura de um lado contra busca 1 do outro",
            ),
        ),
        (
            "regime_de_cache",
            phxsql_core::json::Json::texto_de(
                "os dois lados medidos QUENTES: colmeia com o arquivo inteiro em \
                 memoria desde o abrir; Padrao com definir_cache_paginas(1_000_000) \
                 e uma passada de aquecimento antes do cronometro",
            ),
        ),
        (
            "maquina",
            phxsql_core::json::Json::texto_de(maquina.clone()),
        ),
        (
            "carga_1min_no_inicio",
            phxsql_core::json::Json::Numero(carga1min),
        ),
        (
            "combinacoes",
            phxsql_core::json::Json::Lista(resultados_json),
        ),
        (
            "medido_em",
            phxsql_core::json::Json::texto_de(agora_utc_min()),
        ),
    ]);

    std::fs::write(&alvo, raiz_json.escrever_identado()).expect("gravar resultados.json");
    println!("gravado em {}", alvo.display());
}
