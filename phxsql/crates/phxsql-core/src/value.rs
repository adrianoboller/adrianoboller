//! Valores e sua gravacao dentro do slot de largura fixa do `.reg`.

use crate::error::{PhxError, Result};
use crate::types::{ColumnType, PONTEIRO_LEN};

/// Um valor de coluna do PhxSql.
///
/// `Bin` e `Memo` carregam o conteudo em memoria; ao gravar, a camada de
/// armazenamento troca o conteudo por um [`Ponteiro`] para o `.bin` / `.memo`.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    Int(i64),
    UInt(u64),
    Real(f64),
    /// Decimal exato ja escalado (ex.: 12.34 com escala 2 vira 1234).
    Decimal(i128),
    /// Dias desde 1970-01-01.
    Date(i32),
    /// Centesimos de segundo desde a meia-noite.
    Time(i32),
    /// Milissegundos desde 1970-01-01T00:00:00Z.
    DateTime(i64),
    Str(String),
    Bin(Vec<u8>),
    Memo(String),
}

impl Value {
    pub fn e_null(&self) -> bool {
        matches!(self, Value::Null)
    }

    pub fn como_i64(&self) -> Option<i64> {
        match self {
            Value::Int(v) => Some(*v),
            Value::UInt(v) => i64::try_from(*v).ok(),
            Value::Date(v) | Value::Time(v) => Some(*v as i64),
            Value::DateTime(v) => Some(*v),
            Value::Bool(b) => Some(*b as i64),
            _ => None,
        }
    }

    pub fn como_str(&self) -> Option<&str> {
        match self {
            Value::Str(s) | Value::Memo(s) => Some(s.as_str()),
            _ => None,
        }
    }
}

/// Ponteiro de 16 bytes gravado no `.reg` apontando para um bloco do
/// `.bin` ou do `.memo`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Ponteiro {
    /// Offset absoluto do cabecalho do bloco dentro do arquivo externo.
    pub offset: u64,
    /// Tamanho do conteudo em bytes.
    pub tamanho: u32,
    /// CRC-32 do conteudo, conferido a cada leitura.
    pub crc: u32,
}

impl Ponteiro {
    pub const VAZIO: Ponteiro = Ponteiro {
        offset: 0,
        tamanho: 0,
        crc: 0,
    };

    pub fn e_vazio(&self) -> bool {
        self.offset == 0 && self.tamanho == 0
    }

    pub fn escrever(&self, dst: &mut [u8]) -> Result<()> {
        if dst.len() < PONTEIRO_LEN {
            return Err(PhxError::Corrompido(
                "espaco insuficiente para ponteiro".into(),
            ));
        }
        dst[0..8].copy_from_slice(&self.offset.to_le_bytes());
        dst[8..12].copy_from_slice(&self.tamanho.to_le_bytes());
        dst[12..16].copy_from_slice(&self.crc.to_le_bytes());
        Ok(())
    }

    pub fn ler(src: &[u8]) -> Result<Ponteiro> {
        if src.len() < PONTEIRO_LEN {
            return Err(PhxError::Corrompido("ponteiro truncado".into()));
        }
        Ok(Ponteiro {
            offset: u64::from_le_bytes(src[0..8].try_into().unwrap()),
            tamanho: u32::from_le_bytes(src[8..12].try_into().unwrap()),
            crc: u32::from_le_bytes(src[12..16].try_into().unwrap()),
        })
    }
}

fn erro_tipo(esperado: &str, achou: &Value) -> PhxError {
    PhxError::Tipo(format!("esperado {esperado}, recebido {achou:?}"))
}

fn cabe_i64(v: i64, bits: u32) -> Result<()> {
    let min = -(1i128 << (bits - 1));
    let max = (1i128 << (bits - 1)) - 1;
    if (v as i128) < min || (v as i128) > max {
        return Err(PhxError::LimiteExcedido(format!(
            "{v} nao cabe em inteiro de {bits} bits"
        )));
    }
    Ok(())
}

fn cabe_u64(v: u64, bits: u32) -> Result<()> {
    if bits < 64 && v > (1u64 << bits) - 1 {
        return Err(PhxError::LimiteExcedido(format!(
            "{v} nao cabe em inteiro sem sinal de {bits} bits"
        )));
    }
    Ok(())
}

/// Grava o valor, em little-endian, nos `ty.largura()` bytes de `dst`.
///
/// `Value::Null` grava zeros: quem marca a ausencia e o bitmap de nulos do
/// slot, nao o conteudo.
pub fn escrever_inline(valor: &Value, ty: &ColumnType, dst: &mut [u8]) -> Result<()> {
    let largura = ty.largura();
    if dst.len() < largura {
        return Err(PhxError::Corrompido(format!(
            "espaco insuficiente: precisa de {largura}, tem {}",
            dst.len()
        )));
    }
    let dst = &mut dst[..largura];
    dst.fill(0);

    if valor.e_null() {
        return Ok(());
    }

    match ty {
        ColumnType::Bool => {
            let b = match valor {
                Value::Bool(b) => *b,
                Value::Int(i) => *i != 0,
                outro => return Err(erro_tipo("Bool", outro)),
            };
            dst[0] = b as u8;
        }
        ColumnType::Int1 | ColumnType::Int2 | ColumnType::Int4 | ColumnType::Int8 => {
            let v = valor.como_i64().ok_or_else(|| erro_tipo("Int", valor))?;
            cabe_i64(v, (largura * 8) as u32)?;
            dst.copy_from_slice(&v.to_le_bytes()[..largura]);
        }
        ColumnType::UInt1 | ColumnType::UInt2 | ColumnType::UInt4 | ColumnType::UInt8 => {
            let v = match valor {
                Value::UInt(v) => *v,
                Value::Int(i) if *i >= 0 => *i as u64,
                outro => return Err(erro_tipo("UInt", outro)),
            };
            cabe_u64(v, (largura * 8) as u32)?;
            dst.copy_from_slice(&v.to_le_bytes()[..largura]);
        }
        ColumnType::Real4 => {
            let v = match valor {
                Value::Real(v) => *v as f32,
                Value::Int(i) => *i as f32,
                outro => return Err(erro_tipo("Real", outro)),
            };
            dst.copy_from_slice(&v.to_le_bytes());
        }
        ColumnType::Real8 => {
            let v = match valor {
                Value::Real(v) => *v,
                Value::Int(i) => *i as f64,
                outro => return Err(erro_tipo("Real", outro)),
            };
            dst.copy_from_slice(&v.to_le_bytes());
        }
        ColumnType::Decimal { .. } => {
            let v = match valor {
                Value::Decimal(v) => *v,
                Value::Int(i) => *i as i128,
                outro => return Err(erro_tipo("Decimal", outro)),
            };
            dst.copy_from_slice(&v.to_le_bytes());
        }
        ColumnType::Date => {
            let v = match valor {
                Value::Date(v) => *v,
                Value::Int(i) => i32::try_from(*i)
                    .map_err(|_| PhxError::LimiteExcedido(format!("data fora de faixa: {i}")))?,
                outro => return Err(erro_tipo("Date", outro)),
            };
            dst.copy_from_slice(&v.to_le_bytes());
        }
        ColumnType::Time => {
            let v = match valor {
                Value::Time(v) => *v,
                Value::Int(i) => i32::try_from(*i)
                    .map_err(|_| PhxError::LimiteExcedido(format!("hora fora de faixa: {i}")))?,
                outro => return Err(erro_tipo("Time", outro)),
            };
            dst.copy_from_slice(&v.to_le_bytes());
        }
        ColumnType::DateTime => {
            let v = match valor {
                Value::DateTime(v) => *v,
                Value::Int(i) => *i,
                outro => return Err(erro_tipo("DateTime", outro)),
            };
            dst.copy_from_slice(&v.to_le_bytes());
        }
        ColumnType::Str(n) => {
            let s = valor.como_str().ok_or_else(|| erro_tipo("Str", valor))?;
            let bytes = s.as_bytes();
            if bytes.len() > *n as usize {
                return Err(PhxError::LimiteExcedido(format!(
                    "texto de {} bytes nao cabe em Str({n})",
                    bytes.len()
                )));
            }
            dst[..bytes.len()].copy_from_slice(bytes);
        }
        ColumnType::Bin | ColumnType::Memo => {
            return Err(PhxError::Tipo(
                "Bin/Memo nao sao gravados inline; use Ponteiro".into(),
            ));
        }
    }
    Ok(())
}

/// Le de volta um valor gravado por [`escrever_inline`].
pub fn ler_inline(ty: &ColumnType, src: &[u8]) -> Result<Value> {
    let largura = ty.largura();
    if src.len() < largura {
        return Err(PhxError::Corrompido(format!(
            "slot truncado: precisa de {largura}, tem {}",
            src.len()
        )));
    }
    let src = &src[..largura];

    Ok(match ty {
        ColumnType::Bool => Value::Bool(src[0] != 0),
        ColumnType::Int1 => Value::Int(src[0] as i8 as i64),
        ColumnType::Int2 => Value::Int(i16::from_le_bytes(src.try_into().unwrap()) as i64),
        ColumnType::Int4 => Value::Int(i32::from_le_bytes(src.try_into().unwrap()) as i64),
        ColumnType::Int8 => Value::Int(i64::from_le_bytes(src.try_into().unwrap())),
        ColumnType::UInt1 => Value::UInt(src[0] as u64),
        ColumnType::UInt2 => Value::UInt(u16::from_le_bytes(src.try_into().unwrap()) as u64),
        ColumnType::UInt4 => Value::UInt(u32::from_le_bytes(src.try_into().unwrap()) as u64),
        ColumnType::UInt8 => Value::UInt(u64::from_le_bytes(src.try_into().unwrap())),
        ColumnType::Real4 => Value::Real(f32::from_le_bytes(src.try_into().unwrap()) as f64),
        ColumnType::Real8 => Value::Real(f64::from_le_bytes(src.try_into().unwrap())),
        ColumnType::Decimal { .. } => Value::Decimal(i128::from_le_bytes(src.try_into().unwrap())),
        ColumnType::Date => Value::Date(i32::from_le_bytes(src.try_into().unwrap())),
        ColumnType::Time => Value::Time(i32::from_le_bytes(src.try_into().unwrap())),
        ColumnType::DateTime => Value::DateTime(i64::from_le_bytes(src.try_into().unwrap())),
        ColumnType::Str(_) => {
            let fim = src.iter().position(|&b| b == 0).unwrap_or(src.len());
            let s = std::str::from_utf8(&src[..fim]).map_err(|e| {
                PhxError::Corrompido(format!("texto nao e UTF-8 valido no .reg: {e}"))
            })?;
            Value::Str(s.to_string())
        }
        ColumnType::Bin | ColumnType::Memo => {
            return Err(PhxError::Tipo(
                "Bin/Memo nao sao lidos inline; resolva o Ponteiro".into(),
            ));
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roundtrip(ty: ColumnType, v: Value) -> Value {
        let mut buf = vec![0u8; ty.largura()];
        escrever_inline(&v, &ty, &mut buf).unwrap();
        ler_inline(&ty, &buf).unwrap()
    }

    #[test]
    fn inteiros() {
        assert_eq!(
            roundtrip(ColumnType::Int4, Value::Int(-12345)),
            Value::Int(-12345)
        );
        assert_eq!(
            roundtrip(ColumnType::Int1, Value::Int(-128)),
            Value::Int(-128)
        );
        assert_eq!(
            roundtrip(ColumnType::Int8, Value::Int(i64::MIN)),
            Value::Int(i64::MIN)
        );
        assert_eq!(
            roundtrip(ColumnType::UInt2, Value::UInt(65535)),
            Value::UInt(65535)
        );
    }

    #[test]
    fn estouro_de_faixa_e_erro() {
        let ty = ColumnType::Int1;
        let mut buf = vec![0u8; ty.largura()];
        assert!(escrever_inline(&Value::Int(128), &ty, &mut buf).is_err());
        assert!(escrever_inline(&Value::Int(-129), &ty, &mut buf).is_err());
    }

    #[test]
    fn texto_fixo_preenche_com_nul() {
        let ty = ColumnType::Str(10);
        let mut buf = vec![0xFFu8; 10];
        escrever_inline(&Value::Str("Adriano".into()), &ty, &mut buf).unwrap();
        assert_eq!(&buf[7..], &[0, 0, 0]);
        assert_eq!(ler_inline(&ty, &buf).unwrap(), Value::Str("Adriano".into()));
    }

    #[test]
    fn texto_maior_que_a_coluna_e_erro() {
        let ty = ColumnType::Str(4);
        let mut buf = vec![0u8; 4];
        assert!(escrever_inline(&Value::Str("cadastro".into()), &ty, &mut buf).is_err());
    }

    #[test]
    fn null_grava_zeros() {
        let ty = ColumnType::Int8;
        let mut buf = vec![0xAAu8; 8];
        escrever_inline(&Value::Null, &ty, &mut buf).unwrap();
        assert_eq!(buf, vec![0u8; 8]);
    }

    #[test]
    fn ponteiro_roundtrip() {
        let p = Ponteiro {
            offset: 4096,
            tamanho: 1234,
            crc: 0xDEAD_BEEF,
        };
        let mut buf = [0u8; PONTEIRO_LEN];
        p.escrever(&mut buf).unwrap();
        assert_eq!(Ponteiro::ler(&buf).unwrap(), p);
    }
}
