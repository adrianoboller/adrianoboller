//! # PhxZip -- o 7-Zip em Rust desta casa
//!
//! Le e grava o formato 7z com o conjunto atual dele, e so ele (pedido 454):
//! **Copy, LZMA, LZMA2 e 7zAES** (AES-256 com chave de SHA-256 iterado).
//! Deflate, BZip2, PPMd, os filtros BCJ e tudo de ZIP sao recusa com nome
//! ([`Erro::MetodoRecusado`]) -- sem codigo morto de formato descontinuado.
//!
//! Tudo o que esta aqui foi escrito aqui: LZMA/LZMA2 nos dois sentidos, AES,
//! o conteiner. O CRC-32 e o SHA-256 sao os do PhxSql, pela `phxhash`, e nao
//! uma segunda copia. Zero dependencia externa.
//!
//! # Onde roda
//!
//! `#![no_std]` com `alloc`: a crate trabalha sobre bytes em memoria, sem
//! arquivo, relogio nem acaso do sistema. Quem chama le o arquivo, da a data
//! e fornece a semente do IV. E o que a deixa rodar em Windows, Linux, macOS,
//! Android, iOS e microcontrolador (pedido 450) -- e o que cada alvo prova
//! esta em `docs/PHXZIP.md`.
//!
//! # Exemplo
//!
//! ```
//! use phxzip::{Arquivo7z, Escritor, Limites, Opcoes};
//!
//! let mut e = Escritor::novo(Opcoes { senha: Some("segredo".into()), acaso: [9; 32], ..Opcoes::default() });
//! e.arquivo("config.json", b"{\"porta\": 5433}".to_vec(), None, None).unwrap();
//! let bytes = e.gravar().unwrap();
//!
//! let a = Arquivo7z::abrir(&bytes, Some("segredo"), Limites::default()).unwrap();
//! assert_eq!(a.entradas()[0].nome, "config.json");
//! assert_eq!(a.extrair(0).unwrap(), b"{\"porta\": 5433}");
//! ```

#![cfg_attr(not(any(test, feature = "std")), no_std)]
#![warn(missing_docs)]

extern crate alloc;
#[cfg(feature = "std")]
extern crate std;

pub mod aes;
pub mod caminho;
pub mod chave;
mod erro;
pub mod escritor;
pub mod formato;
pub mod leitor;
pub mod lzma;

pub use caminho::caminho_seguro;
pub use erro::{Erro, Resultado};
#[cfg(feature = "std")]
pub use escritor::fios_padrao;
pub use escritor::{bloco_padrao, Escritor, Opcoes};
pub use formato::{filetime_de_unix, unix_de_filetime};
pub use leitor::{Arquivo7z, Entrada, Limites};
