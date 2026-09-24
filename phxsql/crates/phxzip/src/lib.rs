//! # PhxZip -- o 7-Zip em Rust, zero dependencias
//!
//! Decisao do dono, 24/09/2026: *«PhxZip e o 7zip em Rust»*. Nasceu do pedido
//! 450 (os JSON de configuracao gravados como `.phz`) e e componente dos tres
//! pilares -- PhxSql, PhxMail e Phxblockchain.
//!
//! O fonte do 7-Zip e CONSULTA, nao dependencia (`docs/7ZIP.md`): o que esta em
//! `C/` com «Public domain» no cabecalho foi lido e reescrito contra as nossas
//! restricoes; o que esta em `CPP/` e LGPL e foi so lido, para entender o
//! algoritmo. Cada modulo cita o arquivo e a linha que o inspirou.
//!
//! ## Portavel por construcao
//!
//! `#![no_std]` + `alloc`: a crate trabalha sobre fatias de bytes e nao abre
//! arquivo, nao le relogio e nao pede numero aleatorio ao sistema -- quem abre o
//! arquivo e o chamador. Todo inteiro do arquivo e lido por `from_le_bytes`, e
//! todo tamanho que vem dele como `u64` passa por `usize::try_from` com erro
//! nomeado, para que num alvo de 32 bits um tamanho de 5 GB nao vire truncamento
//! calado. Sem `unsafe` e sem instrucao de CPU.
//!
//! O que falta para os alvos sem sistema operacional esta dito sem arredondar:
//! o CRC-32 e o SHA-256 ainda vem do `phxsql-core`, que usa `std`. O codigo
//! DESTA crate nao usa `std` (so o `impl std::error::Error`, atras do recurso
//! `std`, ligado por padrao); o alvo `no_std` compila no dia em que o `crc.rs`
//! e o `hash.rs` sairem para uma crate `no_std`.
//!
//! ## Plataformas, medidas em 24/09/2026
//!
//! | alvo | o que se provou |
//! |---|---|
//! | `x86_64-unknown-linux-gnu` | testes rodando |
//! | `armv7-unknown-linux-musleabihf` (Raspberry 32 bits) | testes rodando sob `qemu-arm` |
//! | `aarch64-unknown-linux-musl` (Raspberry 64 bits) | testes rodando sob `qemu-aarch64` |
//! | `x86_64-pc-windows-gnu` | testes rodando sob `wine` |
//! | `aarch64-linux-android` | compila e liga (NDK); nao roda aqui |
//! | `s390x-unknown-linux-gnu` (big-endian) | so compila, testes inclusive; nao liga nem roda aqui |
//! | `aarch64-unknown-linux-gnu`, macOS (`aarch64`/`x86_64-apple-darwin`), iOS | so `cargo check`: NAO MEDIDO rodando |
//! | `riscv32imc`/`riscv32imac-unknown-none-elf` (ESP32-C3/C6), `thumbv7em-none-eabihf`, `thumbv6m-none-eabi` | NAO compila: o `phxsql-core` pede `std` |
//! | ESP32 classico/S2/S3 (Xtensa), Arduino AVR | NAO MEDIDO: sem alvo no Rust estavel; AVR tem 2 a 8 KB de RAM |
//!
//! O mesmo `.phz` sai com os MESMOS bytes em toda plataforma que roda (o IV e
//! sintetico e todo inteiro e little-endian): o teste
//! `o_arquivo_gravado_e_o_mesmo_em_toda_plataforma` confere o SHA-256.
//!
//! ## Para quem abre arquivo que veio de fora, e para quem extrai
//!
//! O que o motor garante (pedido 471, parecer SEC de 24/09/2026):
//!
//! * [`Limites::default`] e o de entrada NAO confiavel -- ciclos do 7zAES ate
//!   19, duas derivacoes por abertura, 65.536 entradas, cabecalho de 8 MiB --,
//!   e tudo o que o arquivo declara e conferido contra ele antes de derivar ou
//!   de alocar. [`Limites::confiavel`] e o ato escrito de abrir com a folga do
//!   7-Zip.
//! * Todo nome passa por [`conferir_nome`] -- nada de caminho absoluto, `..`,
//!   letra de unidade, NUL, dispositivo do Windows ou ponto/espaco no fim -- e
//!   nome repetido (sem diferenca de caixa) recusa o arquivo inteiro.
//!
//! O que o motor NAO faz, e o extrator nao pode fazer por ele:
//!
//! * **`Entrada::atributos` e informacao, nao ordem.** O 7z pode carregar nos
//!   bits altos o modo Unix, inclusive o de LINK SIMBOLICO. O motor so entrega
//!   bytes e nunca cria link; o extrator nao pode honrar esses bits como link
//!   nem como permissao -- um link extraido e seguido depois e o zip-slip por
//!   outra porta.
//! * **Sem zeroizacao** de chave e senha na memoria (ver `phz.rs`).
//! * **Colisao entre pasta e arquivo** (`a` arquivo e `a/b`) e **normalizacao
//!   Unicode** (o macOS compoe `ç` diferente) nao sao conferidas: dao erro ao
//!   gravar, nao sobrescrita calada, e ficam com o extrator.

#![no_std]

extern crate alloc;
#[cfg(any(test, feature = "std"))]
extern crate std;

pub mod aes;
pub mod chave;
pub mod erro;
pub mod escritor;
pub mod leitor;
pub mod lzma;
pub mod lzma_compressor;
pub mod phz;

pub use chave::{compressoes, CICLOS_MAXIMO, CICLOS_PADRAO};
pub use erro::Erro;
pub use escritor::{Escritor, Metodo, Opcoes};
pub use leitor::{conferir_nome, Arquivo, Entrada, Limites};
pub use phz::{
    desempacotar, desempacotar_com_limites, desempacotar_com_teto, empacotar, empacotar_com_ciclos,
    EXTENSAO,
};

/// Segundos entre 1601-01-01 (a epoca do `FILETIME`) e 1970-01-01.
const SEGUNDOS_1601_A_1970: i64 = 11_644_473_600;

/// Segundos Unix para o `FILETIME` que o 7z grava (intervalos de 100 ns desde
/// 1601). Antes de 1601 nao ha o que representar, e vira zero.
pub fn unix_para_filetime(segundos: i64) -> u64 {
    let s = segundos.saturating_add(SEGUNDOS_1601_A_1970).max(0) as u64;
    s.saturating_mul(10_000_000)
}

/// `FILETIME` para segundos Unix, arredondando para baixo.
pub fn filetime_para_unix(ft: u64) -> i64 {
    (ft / 10_000_000) as i64 - SEGUNDOS_1601_A_1970
}
