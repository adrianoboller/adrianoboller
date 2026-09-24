//! Quanto o `Arquivo::abrir` ALOCA diante de um cabecalho hostil -- medido por
//! um alocador que conta, e nao estimado.
//!
//! Pedido 471 (parecer SEC de 24/09/2026): as contagens do cabecalho so eram
//! conferidas contra os bytes que restavam, e os vetores dimensionados por
//! elas custavam ~150 bytes por byte de cabecalho. Com o cabecalho COMPRIMIDO,
//! um arquivo de poucos KB vira um cabecalho do tamanho do teto -- e o teto,
//! multiplicado por 150. O que estes testes provam e o contrario: o pico de
//! abrir fica proporcional aos `Limites`, e nao ao que o arquivo declara.
//!
//! # O alocador, e por que ha `unsafe` aqui
//!
//! Medir alocacao sem crate de fora exige um `GlobalAlloc` proprio, e o trait
//! e `unsafe` por definicao. Ele so repassa ao `System` e soma os tamanhos; a
//! crate `phxzip` continua sem `unsafe` nenhum. Os testes deste arquivo rodam
//! um de cada vez (a trava `UM_POR_VEZ`), senao o pico de um somaria a
//! alocacao do outro -- cada arquivo em `tests/` e um processo, entao os
//! testes dos outros arquivos nao entram na conta.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

use phxzip::{Arquivo, Erro, Escritor, Limites, Metodo, Opcoes};

struct Contador;

static ATUAL: AtomicUsize = AtomicUsize::new(0);
static PICO: AtomicUsize = AtomicUsize::new(0);
static UM_POR_VEZ: Mutex<()> = Mutex::new(());

fn somar(n: usize) {
    let agora = ATUAL.fetch_add(n, Ordering::SeqCst) + n;
    PICO.fetch_max(agora, Ordering::SeqCst);
}

unsafe impl GlobalAlloc for Contador {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        let p = System.alloc(l);
        if !p.is_null() {
            somar(l.size());
        }
        p
    }

    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        System.dealloc(p, l);
        ATUAL.fetch_sub(l.size(), Ordering::SeqCst);
    }

    unsafe fn realloc(&self, p: *mut u8, l: Layout, novo: usize) -> *mut u8 {
        let q = System.realloc(p, l, novo);
        if !q.is_null() {
            if novo > l.size() {
                somar(novo - l.size());
            } else {
                ATUAL.fetch_sub(l.size() - novo, Ordering::SeqCst);
            }
        }
        q
    }
}

#[global_allocator]
static ALOCADOR: Contador = Contador;

/// O pico de alocacao durante `f`, acima do que ja estava alocado antes.
fn pico_durante<T>(f: impl FnOnce() -> T) -> (T, usize) {
    let base = ATUAL.load(Ordering::SeqCst);
    PICO.store(base, Ordering::SeqCst);
    let r = f();
    (r, PICO.load(Ordering::SeqCst) - base)
}

/// O `UINT64` do 7z (o mesmo que o escritor grava).
fn numero(v: &mut Vec<u8>, n: u64) {
    for extras in 0..8u32 {
        if n < 1u64 << (7 * (extras + 1)) {
            v.push(!(0xFFu8 >> extras) | (n >> (8 * extras)) as u8);
            v.extend_from_slice(&n.to_le_bytes()[..extras as usize]);
            return;
        }
    }
    v.push(0xFF);
    v.extend_from_slice(&n.to_le_bytes());
}

/// Um 7z cujo cabecalho e `claro`, comprimido em LZMA2 (`kEncodedHeader`) --
/// o caminho em que poucos KB de arquivo viram o cabecalho inteiro.
fn com_cabecalho_comprimido(claro: &[u8]) -> Vec<u8> {
    let prop = phxzip::lzma_compressor::propriedade_do_dicionario(claro.len());
    let comprimido = phxzip::lzma_compressor::lzma2(claro, prop);
    let mut reg = Vec::new();
    reg.push(0x17); // kEncodedHeader
    reg.push(0x06); // kPackInfo
    numero(&mut reg, 0);
    numero(&mut reg, 1);
    reg.push(0x09);
    numero(&mut reg, comprimido.len() as u64);
    reg.push(0x00);
    reg.extend_from_slice(&[0x07, 0x0B]); // kUnPackInfo, kFolder
    numero(&mut reg, 1);
    reg.push(0x00);
    numero(&mut reg, 1); // um coder
    reg.extend_from_slice(&[0x21, 0x21, 0x01, prop]); // LZMA2, 1 byte de propriedade
    reg.push(0x0C);
    numero(&mut reg, claro.len() as u64);
    reg.extend_from_slice(&[0x0A, 0x01]);
    reg.extend_from_slice(&phxsql_core::crc32(claro).to_le_bytes());
    reg.push(0x00);
    reg.push(0x00);

    let mut inicio = Vec::new();
    inicio.extend_from_slice(&(comprimido.len() as u64).to_le_bytes());
    inicio.extend_from_slice(&(reg.len() as u64).to_le_bytes());
    inicio.extend_from_slice(&phxsql_core::crc32(&reg).to_le_bytes());
    let mut arq = b"7z\xbc\xaf\x27\x1c\x00\x04".to_vec();
    arq.extend_from_slice(&phxsql_core::crc32(&inicio).to_le_bytes());
    arq.extend_from_slice(&inicio);
    arq.extend_from_slice(&comprimido);
    arq.extend_from_slice(&reg);
    arq
}

/// O ataque do parecer: um cabecalho comprimido que declara a MAIOR contagem
/// de entradas que o cabecalho descompactado comporta. Antes do conserto,
/// abrir alocava ~150 bytes por entrada declarada antes de achar o primeiro
/// erro; agora a contagem passa pelo teto `Limites::entradas` antes de
/// qualquer vetor, e o pico e o proprio cabecalho descompactado.
#[test]
fn contagem_do_cabecalho_comprimido_nao_aloca_pelo_que_declara() {
    let _vez = UM_POR_VEZ.lock().unwrap_or_else(|e| e.into_inner());
    const S: usize = 200_000;
    let mut claro = vec![0x01, 0x05]; // kHeader, kFilesInfo
    numero(&mut claro, (S - 16) as u64);
    claro.resize(S, 0);
    let arq = com_cabecalho_comprimido(&claro);
    drop(claro);
    let limites = Limites {
        cabecalho: 256 << 10,
        ..Limites::default()
    };
    let (r, pico) = pico_durante(|| Arquivo::abrir(&arq, None, limites).map(|_| ()));
    println!(
        "pico medido: {pico} bytes para {} bytes de arquivo",
        arq.len()
    );
    assert!(
        pico <= 2 * S + (256 << 10),
        "abrir {} bytes de arquivo alocou {pico} bytes de pico, para um cabecalho de {S} \
         ({:.0}x o cabecalho)",
        arq.len(),
        pico as f64 / S as f64
    );
    assert!(
        matches!(
            r,
            Err(Erro::GrandeDemais {
                oque: "quantidade de entradas",
                ..
            })
        ),
        "veio {r:?}"
    );
}

/// O cabecalho gravado EM CLARO tambem obedece a `Limites::cabecalho`: acima
/// do teto, e recusado antes de ser analisado. Sem isso, um cabecalho de
/// 180 KB passava por baixo de um teto de 16 KiB e cada entrada dele virava
/// alocacao.
#[test]
fn cabecalho_plano_acima_do_teto_nao_e_analisado() {
    let _vez = UM_POR_VEZ.lock().unwrap_or_else(|e| e.into_inner());
    let mut e = Escritor::novo(Opcoes {
        metodo: Metodo::Copia,
        senha: None,
        ciclos: 4,
        cifrar_cabecalho: false,
    })
    .unwrap();
    for i in 0..3000 {
        e.arquivo(&format!("arquivo-{i:05}.txt"), b"x", None)
            .unwrap();
    }
    let arq = e.terminar();
    let tam_cab = u64::from_le_bytes(arq[20..28].try_into().unwrap());
    let limites = Limites {
        cabecalho: 16 << 10,
        entradas: 1 << 20,
        ..Limites::default()
    };
    let (r, pico) = pico_durante(|| Arquivo::abrir(&arq, None, limites).map(|_| ()));
    println!("pico medido: {pico} bytes para um cabecalho de {tam_cab}");
    assert!(
        pico <= 64 << 10,
        "um cabecalho em claro de {tam_cab} bytes, com teto de 16 KiB, alocou {pico} bytes de pico"
    );
    assert_eq!(
        r,
        Err(Erro::GrandeDemais {
            oque: "cabecalho",
            declarado: tam_cab,
            teto: 16 << 10
        })
    );
}
