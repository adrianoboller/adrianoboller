//! # phxsql-core
//!
//! Fundacao do PhxSql: tipos de coluna, valores, esquema, codificacao de
//! chaves e CRC. Sem dependencias externas -- compila offline.
//!
//! O PhxSql organiza cada tabela logica em quatro arquivos fisicos, no
//! espirito do HFSQL:
//!
//! ```text
//! cadastroClientes.reg   registros na ordem de digitacao
//! cadastroClientes.ndx   indices (B+tree)
//! cadastroClientes.bin   binarios
//! cadastroClientes.memo  textos longos
//! ```
//!
//! Os quatro juntos formam a tabela de dados `cadastroClientes`.

pub mod crc;
pub mod datahora;
pub mod error;
pub mod keyenc;
pub mod schema;
pub mod types;
pub mod value;

pub use crc::{crc32, crc32_with};
pub use error::{PhxError, Result};
pub use schema::{Column, IndexColumn, IndexDef, Schema};
pub use types::{ColumnType, PONTEIRO_LEN};
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
