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
pub mod fio;
pub mod frogcript;
pub mod hash;
pub mod hkdf;
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
pub mod x25519;
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

/// A proveniencia deste build, numa linha -- versao, commit e alvo.
///
/// Existe porque `--version` precisava responder a mesma coisa nos tres
/// binarios, e porque versao sem commit nao identifica build nenhum: dois
/// pacotes com `0.18.0` no nome podem ser arvores diferentes.
pub fn versao_completa(programa: &str) -> String {
    let commit = env!("PHX_COMMIT");
    let curto = if commit.is_empty() {
        "desconhecido".to_string()
    } else {
        let mut c = commit[..commit.len().min(12)].to_string();
        // Arvore suja e informacao de producao: o binario NAO corresponde ao
        // commit que anuncia, e quem depura precisa saber disso.
        if env!("PHX_SUJO") == "1" {
            c.push_str("-sujo");
        }
        c
    };
    let alvo = env!("PHX_ALVO");
    let alvo = if alvo.is_empty() { "" } else { alvo };
    format!(
        "{programa} {} ({curto}){}",
        env!("CARGO_PKG_VERSION"),
        if alvo.is_empty() {
            String::new()
        } else {
            format!(" {alvo}")
        }
    )
}
