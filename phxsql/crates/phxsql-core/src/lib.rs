//! # phxsql-core
//!
//! Fundacao do PhxSql: tipos de coluna, valores, esquema, codificacao de
//! chaves e CRC. Sem dependencias externas -- compila offline.
//!
//! O PhxSql organiza cada tabela logica em quatro arquivos fisicos, no
//! espirito do HFSQL(R):
//!
//! ```text
//! cadastroClientes.reg   registros na ordem de digitacao
//! cadastroClientes.ndx   indices (B+tree)
//! cadastroClientes.bin   binarios
//! cadastroClientes.memo  textos longos
//! ```
//!
//! Os quatro juntos formam a tabela de dados `cadastroClientes`.

pub mod base64;
pub mod carga;
pub mod cifra;
pub mod crc;
pub mod datahora;
pub mod desafio;
pub mod ed25519;
pub mod error;
pub mod frogcript;
pub mod hash;
pub mod json;
pub mod keyenc;
pub mod paginacao;
pub mod paralelo;
pub mod schema;
pub mod senha;
pub mod sha1;
pub mod sha512;
pub mod types;
pub mod uuid;
pub mod value;
pub mod zip;

pub use cifra::{abrir, selar, Sequencia, CHAVE_LEN, NONCE_LEN, TAG_LEN};
pub use crc::{crc32, crc32_with};
pub use error::{PhxError, Result};
pub use schema::{Column, IndexColumn, IndexDef, Schema};
pub use types::{ColumnType, DadoPessoal, PONTEIRO_LEN};
pub use uuid::{Uuid, Uuid256};
pub use value::{escrever_inline, ler_inline, Ponteiro, Value};

/// Identificador fisico de um registro dentro do `.reg`.
///
/// E o numero do slot, comecando em 1. Como o `.reg` nunca reordena
/// registros, o rowid tambem e a ordem de digitacao.
pub type RowId = u64;

/// Extensoes dos quatro arquivos que compoem uma tabela.
pub const EXT_REG: &str = "reg";
pub const EXT_NDX: &str = "ndx";
pub const EXT_BIN: &str = "bin";
pub const EXT_MEMO: &str = "memo";
/// Espelho do `.reg`, quando ligado. Ver `volume::Volumes::com_espelho`.
pub const EXT_BKP: &str = "bkp";
