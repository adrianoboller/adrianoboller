//! # phxhash
//!
//! CRC-32 e SHA-256 (com HMAC e PBKDF2) escritos aqui, sem `std`.
//!
//! Moravam no `phxsql-core`. Sairam porque o PhxZip roda em microcontrolador
//! (pedido 450: Arduino ARM, ESP32 RISC-V) e la nao ha sistema operacional --
//! so `core` e `alloc`. O `phxsql-core` reexporta os dois modulos com o mesmo
//! nome, entao nenhum `phxsql_core::crc::crc32` do repositorio mudou: o motor
//! e um so, e quem chama nao sabe que ele mudou de casa.
//!
//! Os testes rodam com `std` (o `catch_unwind` de um deles exige); o codigo de
//! producao compila sem.

#![cfg_attr(not(test), no_std)]

extern crate alloc;

pub mod crc;
pub mod hash;
