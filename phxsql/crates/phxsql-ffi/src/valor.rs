//! `PhxValor`: um `Value` do motor numa struct que o C entende.
//!
//! # Por que uma struct etiquetada, e nao JSON
//!
//! Porque JSON nao carrega binario. Uma coluna `Bin` e bytes crus por
//! definicao, e para passar por JSON teria de virar base64 -- um desvio que
//! custa 33% de tamanho e uma codificacao a mais para errar, numa fronteira
//! que existe justamente para ser barata. A struct passa o ponteiro.
//!
//! # E por que o u64 mora no campo com sinal
//!
//! Um `u64` acima de `i64::MAX` nao cabe num `i64`, mas cabe nos MESMOS 64
//! bits. Entao `PHX_UINT` guarda o padrao de bits em `numero` e o le de volta
//! com `as u64`: exato, sem campo extra e sem perder o topo da faixa. Esta
//! escrito no cabecalho de C, porque quem le o campo sem saber disso ve um
//! numero negativo e acha que achou um defeito.

use phxsql_core::types::ColumnType;
use phxsql_core::value::Value;
use phxsql_core::{Uuid, Uuid256};

use crate::erro::{anotar, PHX_ERRO_PONTEIRO, PHX_ERRO_USO, PHX_ERRO_UTF8};

pub const PHX_NULO: i32 = 0;
pub const PHX_BOOL: i32 = 1;
pub const PHX_INT: i32 = 2;
pub const PHX_UINT: i32 = 3;
pub const PHX_REAL: i32 = 4;
pub const PHX_DECIMAL: i32 = 5;
pub const PHX_DATA: i32 = 6;
pub const PHX_HORA: i32 = 7;
pub const PHX_DATAHORA: i32 = 8;
pub const PHX_TEXTO: i32 = 9;
pub const PHX_BIN: i32 = 10;
pub const PHX_MEMO: i32 = 11;
pub const PHX_UUID: i32 = 12;
pub const PHX_UUID256: i32 = 13;

/// Um valor atravessando a fronteira.
///
/// `dados`/`tam` sao EMPRESTADOS nas duas direcoes: na entrada apontam para a
/// memoria do chamador e sao copiados antes de a chamada voltar; na saida
/// apontam para dentro do punho da linha e morrem no `phx_linha_liberar`.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct PhxValor {
    pub tipo: i32,
    /// Sempre zero hoje. Existe para o dia em que faltar um campo: acrescentar
    /// um campo no fim de uma struct de ABI quebra todo binario ja compilado.
    pub reservado: u32,
    /// Inteiro com sinal; para `PHX_UINT`, o padrao de bits do `u64`.
    pub numero: i64,
    pub real: f64,
    pub dados: *const u8,
    pub tam: usize,
}

impl PhxValor {
    fn cru(tipo: i32) -> PhxValor {
        PhxValor {
            tipo,
            reservado: 0,
            numero: 0,
            real: 0.0,
            dados: std::ptr::null(),
            tam: 0,
        }
    }

    fn com_numero(tipo: i32, n: i64) -> PhxValor {
        let mut v = PhxValor::cru(tipo);
        v.numero = n;
        v
    }

    fn com_bytes(tipo: i32, b: &[u8]) -> PhxValor {
        let mut v = PhxValor::cru(tipo);
        v.dados = b.as_ptr();
        v.tam = b.len();
        v
    }
}

/// Le um `PhxValor` do chamador e COPIA para um `Value`.
///
/// Copia de proposito: o `Value` vai para dentro do motor e sobrevive a
/// chamada, enquanto os bytes do chamador podem ser de uma pilha que ja
/// desapareceu na proxima linha do C dele.
///
/// # Safety
///
/// `v.dados`/`v.tam` tem de descrever memoria legivel.
pub unsafe fn para_value(v: &PhxValor) -> Result<Value, i32> {
    let bytes = |v: &PhxValor| -> Result<&[u8], i32> {
        crate::texto::bytes(v.dados, v.tam)
            .ok_or_else(|| anotar(PHX_ERRO_PONTEIRO, "valor com dados nulos e tamanho > 0"))
    };
    let texto = |v: &PhxValor| -> Result<String, i32> {
        let b = bytes(v)?;
        String::from_utf8(b.to_vec())
            .map_err(|_| anotar(PHX_ERRO_UTF8, "texto do valor nao e UTF-8 valido"))
    };
    Ok(match v.tipo {
        PHX_NULO => Value::Null,
        PHX_BOOL => Value::Bool(v.numero != 0),
        PHX_INT => Value::Int(v.numero),
        PHX_UINT => Value::UInt(v.numero as u64),
        PHX_REAL => Value::Real(v.real),
        PHX_DECIMAL => {
            let b = bytes(v)?;
            if b.len() != 16 {
                return Err(anotar(
                    PHX_ERRO_USO,
                    format!("PHX_DECIMAL exige 16 bytes little-endian, veio {}", b.len()),
                ));
            }
            let mut a = [0u8; 16];
            a.copy_from_slice(b);
            Value::Decimal(i128::from_le_bytes(a))
        }
        PHX_DATA => Value::Date(v.numero as i32),
        PHX_HORA => Value::Time(v.numero as i32),
        PHX_DATAHORA => Value::DateTime(v.numero),
        PHX_TEXTO => Value::Str(texto(v)?),
        PHX_BIN => Value::Bin(bytes(v)?.to_vec()),
        PHX_MEMO => Value::Memo(texto(v)?),
        PHX_UUID => {
            let b = bytes(v)?;
            if b.len() != 16 {
                return Err(anotar(
                    PHX_ERRO_USO,
                    format!("PHX_UUID exige 16 bytes, veio {}", b.len()),
                ));
            }
            let mut a = [0u8; 16];
            a.copy_from_slice(b);
            Value::Uuid(Uuid::de_bytes(a))
        }
        PHX_UUID256 => {
            let b = bytes(v)?;
            if b.len() != 32 {
                return Err(anotar(
                    PHX_ERRO_USO,
                    format!("PHX_UUID256 exige 32 bytes, veio {}", b.len()),
                ));
            }
            let mut a = [0u8; 32];
            a.copy_from_slice(b);
            Value::Uuid256(Uuid256::de_bytes(a))
        }
        outro => {
            return Err(anotar(
                PHX_ERRO_USO,
                format!("tipo de valor desconhecido: {outro}"),
            ))
        }
    })
}

/// Monta a vista em C de um `Value` que continua vivo em outro lugar.
///
/// O ponteiro devolvido aponta PARA DENTRO do `Value` recebido -- por isso
/// esta funcao e privada ao punho da linha, que e quem garante que o `Value`
/// nao se move nem morre antes do `liberar`.
pub fn do_value(v: &Value) -> PhxValor {
    match v {
        Value::Null => PhxValor::cru(PHX_NULO),
        Value::Bool(b) => PhxValor::com_numero(PHX_BOOL, *b as i64),
        Value::Int(n) => PhxValor::com_numero(PHX_INT, *n),
        Value::UInt(n) => PhxValor::com_numero(PHX_UINT, *n as i64),
        Value::Real(r) => {
            let mut p = PhxValor::cru(PHX_REAL);
            p.real = *r;
            p
        }
        // O i128 nao cabe em campo nenhum da struct, entao ele viaja como os
        // seus 16 bytes -- e eles precisam de um lugar estavel, que e o
        // proprio punho da linha. Ver `linha.rs`.
        Value::Decimal(_) => PhxValor::cru(PHX_DECIMAL),
        Value::Date(d) => PhxValor::com_numero(PHX_DATA, *d as i64),
        Value::Time(t) => PhxValor::com_numero(PHX_HORA, *t as i64),
        Value::DateTime(t) => PhxValor::com_numero(PHX_DATAHORA, *t),
        Value::Str(s) => PhxValor::com_bytes(PHX_TEXTO, s.as_bytes()),
        Value::Bin(b) => PhxValor::com_bytes(PHX_BIN, b),
        Value::Memo(s) => PhxValor::com_bytes(PHX_MEMO, s.as_bytes()),
        Value::Uuid(u) => PhxValor::com_bytes(PHX_UUID, u.bytes()),
        Value::Uuid256(u) => PhxValor::com_bytes(PHX_UUID256, u.bytes()),
    }
}

// ----------------------------------------------------------------- colunas

pub const PHX_COL_BOOL: i32 = 1;
pub const PHX_COL_INT1: i32 = 2;
pub const PHX_COL_INT2: i32 = 3;
pub const PHX_COL_INT4: i32 = 4;
pub const PHX_COL_INT8: i32 = 5;
pub const PHX_COL_UINT1: i32 = 6;
pub const PHX_COL_UINT2: i32 = 7;
pub const PHX_COL_UINT4: i32 = 8;
pub const PHX_COL_UINT8: i32 = 9;
pub const PHX_COL_REAL4: i32 = 10;
pub const PHX_COL_REAL8: i32 = 11;
pub const PHX_COL_DECIMAL: i32 = 12;
pub const PHX_COL_DATA: i32 = 13;
pub const PHX_COL_HORA: i32 = 14;
pub const PHX_COL_DATAHORA: i32 = 15;
pub const PHX_COL_STR: i32 = 16;
pub const PHX_COL_BIN: i32 = 17;
pub const PHX_COL_MEMO: i32 = 18;
pub const PHX_COL_UUID: i32 = 19;
pub const PHX_COL_UUID256: i32 = 20;
/// Sequencia: o contador da tabela, gravado sozinho quando a coluna chega
/// nula. Uma por tabela.
pub const PHX_COL_SEQUENCIA: i32 = 21;

/// O tipo de coluna que o C pediu. `largura` so vale para `STR`; `precisao` e
/// `escala` so para `DECIMAL`.
pub fn tipo_de_coluna(
    tipo: i32,
    largura: u32,
    precisao: u8,
    escala: u8,
) -> Result<ColumnType, i32> {
    Ok(match tipo {
        PHX_COL_BOOL => ColumnType::Bool,
        PHX_COL_INT1 => ColumnType::Int1,
        PHX_COL_INT2 => ColumnType::Int2,
        PHX_COL_INT4 => ColumnType::Int4,
        PHX_COL_INT8 => ColumnType::Int8,
        PHX_COL_UINT1 => ColumnType::UInt1,
        PHX_COL_UINT2 => ColumnType::UInt2,
        PHX_COL_UINT4 => ColumnType::UInt4,
        PHX_COL_UINT8 => ColumnType::UInt8,
        PHX_COL_REAL4 => ColumnType::Real4,
        PHX_COL_REAL8 => ColumnType::Real8,
        PHX_COL_DECIMAL => ColumnType::Decimal { precisao, escala },
        PHX_COL_DATA => ColumnType::Date,
        PHX_COL_HORA => ColumnType::Time,
        PHX_COL_DATAHORA => ColumnType::DateTime,
        PHX_COL_STR => {
            if largura == 0 || largura > u16::MAX as u32 {
                return Err(anotar(
                    PHX_ERRO_USO,
                    format!("PHX_COL_STR exige largura entre 1 e 65535, veio {largura}"),
                ));
            }
            ColumnType::Str(largura as u16)
        }
        PHX_COL_BIN => ColumnType::Bin,
        PHX_COL_MEMO => ColumnType::Memo,
        PHX_COL_UUID => ColumnType::Uuid,
        PHX_COL_UUID256 => ColumnType::Uuid256,
        PHX_COL_SEQUENCIA => ColumnType::Sequence,
        outro => {
            return Err(anotar(
                PHX_ERRO_USO,
                format!("tipo de coluna desconhecido: {outro}"),
            ))
        }
    })
}

/// O caminho de volta: o tipo da coluna, para quem abriu a tabela e quer
/// saber com o que esta lidando.
pub fn codigo_de_coluna(ty: ColumnType) -> i32 {
    match ty {
        ColumnType::Bool => PHX_COL_BOOL,
        ColumnType::Int1 => PHX_COL_INT1,
        ColumnType::Int2 => PHX_COL_INT2,
        ColumnType::Int4 => PHX_COL_INT4,
        ColumnType::Int8 => PHX_COL_INT8,
        ColumnType::UInt1 => PHX_COL_UINT1,
        ColumnType::UInt2 => PHX_COL_UINT2,
        ColumnType::UInt4 => PHX_COL_UINT4,
        ColumnType::UInt8 => PHX_COL_UINT8,
        ColumnType::Real4 => PHX_COL_REAL4,
        ColumnType::Real8 => PHX_COL_REAL8,
        ColumnType::Decimal { .. } => PHX_COL_DECIMAL,
        ColumnType::Date => PHX_COL_DATA,
        ColumnType::Time => PHX_COL_HORA,
        ColumnType::DateTime => PHX_COL_DATAHORA,
        ColumnType::Str(_) => PHX_COL_STR,
        ColumnType::Bin => PHX_COL_BIN,
        ColumnType::Memo => PHX_COL_MEMO,
        ColumnType::Uuid => PHX_COL_UUID,
        ColumnType::Uuid256 => PHX_COL_UUID256,
        ColumnType::Sequence => PHX_COL_SEQUENCIA,
    }
}
