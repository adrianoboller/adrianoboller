//! Codificacao de chaves preservando ordem, para o `.ndx`.
//!
//! A regra de ouro do indice: comparar as chaves byte a byte (`memcmp`) tem de
//! dar exatamente a mesma ordem que comparar os valores logicos. Isso permite
//! que a B+tree seja totalmente agnostica de tipo -- ela so compara bytes.
//!
//! Cada componente da chave ocupa `1 + largura` bytes:
//!
//! ```text
//! [presenca: 0x00 = NULL, 0x01 = preenchido][bytes ordenaveis do valor]
//! ```
//!
//! Com isso NULL ordena antes de qualquer valor. Em coluna DESC todos os bytes
//! do componente sao invertidos, o que inverte a ordem e joga NULL para o fim.

use crate::error::{PhxError, Result};
use crate::types::ColumnType;
use crate::value::Value;

/// Bytes que um componente de chave ocupa, incluindo o byte de presenca.
pub fn largura_componente(ty: &ColumnType) -> Result<usize> {
    Ok(1 + ty.largura_chave()?)
}

/// Inverte o sinal do bit mais significativo para que inteiros com sinal
/// ordenem corretamente como bytes sem sinal.
fn escrever_int_be(v: i128, largura: usize, dst: &mut [u8]) {
    let bytes = v.to_be_bytes();
    let inicio = bytes.len() - largura;
    dst[..largura].copy_from_slice(&bytes[inicio..]);
    dst[0] ^= 0x80;
}

fn escrever_uint_be(v: u128, largura: usize, dst: &mut [u8]) {
    let bytes = v.to_be_bytes();
    let inicio = bytes.len() - largura;
    dst[..largura].copy_from_slice(&bytes[inicio..]);
}

/// Ponto flutuante ordenavel: negativos tem todos os bits invertidos,
/// positivos tem apenas o bit de sinal ligado.
fn escrever_f64_be(v: f64, dst: &mut [u8]) {
    let bits = v.to_bits();
    let ordenavel = if bits & 0x8000_0000_0000_0000 != 0 {
        !bits
    } else {
        bits | 0x8000_0000_0000_0000
    };
    dst[..8].copy_from_slice(&ordenavel.to_be_bytes());
}

fn escrever_f32_be(v: f32, dst: &mut [u8]) {
    let bits = v.to_bits();
    let ordenavel = if bits & 0x8000_0000 != 0 {
        !bits
    } else {
        bits | 0x8000_0000
    };
    dst[..4].copy_from_slice(&ordenavel.to_be_bytes());
}

/// Grava um componente de chave em `dst`, que precisa ter exatamente
/// [`largura_componente`] bytes.
///
/// * `desc`   -- ordem decrescente (inverte todos os bytes do componente).
/// * `nocase` -- comparacao sem distinguir maiusculas (fold ASCII, como o
///   atributo NOCASE do Clarion(R); nao altera bytes multibyte do UTF-8).
pub fn escrever_componente(
    valor: &Value,
    ty: &ColumnType,
    desc: bool,
    nocase: bool,
    dst: &mut [u8],
) -> Result<()> {
    let total = largura_componente(ty)?;
    if dst.len() != total {
        return Err(PhxError::Corrompido(format!(
            "componente de chave espera {total} bytes, recebeu {}",
            dst.len()
        )));
    }
    dst.fill(0);

    if valor.e_null() {
        if desc {
            for b in dst.iter_mut() {
                *b = !*b;
            }
        }
        return Ok(());
    }

    dst[0] = 0x01;
    let corpo = &mut dst[1..];

    match ty {
        ColumnType::Bool => {
            corpo[0] = match valor {
                Value::Bool(b) => *b as u8,
                Value::Int(i) => (*i != 0) as u8,
                outro => return Err(PhxError::Tipo(format!("esperado Bool, recebido {outro:?}"))),
            };
        }
        ColumnType::Int1 | ColumnType::Int2 | ColumnType::Int4 | ColumnType::Int8 => {
            let v = valor
                .como_i64()
                .ok_or_else(|| PhxError::Tipo(format!("esperado Int, recebido {valor:?}")))?;
            escrever_int_be(v as i128, ty.largura(), corpo);
        }
        ColumnType::UInt1 | ColumnType::UInt2 | ColumnType::UInt4 | ColumnType::UInt8 => {
            let v = match valor {
                Value::UInt(v) => *v,
                Value::Int(i) if *i >= 0 => *i as u64,
                outro => return Err(PhxError::Tipo(format!("esperado UInt, recebido {outro:?}"))),
            };
            escrever_uint_be(v as u128, ty.largura(), corpo);
        }
        ColumnType::Real4 => {
            let v = match valor {
                Value::Real(v) => *v as f32,
                Value::Int(i) => *i as f32,
                outro => return Err(PhxError::Tipo(format!("esperado Real, recebido {outro:?}"))),
            };
            escrever_f32_be(v, corpo);
        }
        ColumnType::Real8 => {
            let v = match valor {
                Value::Real(v) => *v,
                Value::Int(i) => *i as f64,
                outro => return Err(PhxError::Tipo(format!("esperado Real, recebido {outro:?}"))),
            };
            escrever_f64_be(v, corpo);
        }
        ColumnType::Decimal { .. } => {
            let v = match valor {
                Value::Decimal(v) => *v,
                Value::Int(i) => *i as i128,
                outro => {
                    return Err(PhxError::Tipo(format!(
                        "esperado Decimal, recebido {outro:?}"
                    )))
                }
            };
            escrever_int_be(v, 16, corpo);
        }
        ColumnType::Date | ColumnType::Time => {
            let v = valor
                .como_i64()
                .ok_or_else(|| PhxError::Tipo(format!("esperado Date/Time, recebido {valor:?}")))?;
            escrever_int_be(v as i128, 4, corpo);
        }
        ColumnType::DateTime => {
            let v = valor
                .como_i64()
                .ok_or_else(|| PhxError::Tipo(format!("esperado DateTime, recebido {valor:?}")))?;
            escrever_int_be(v as i128, 8, corpo);
        }
        ColumnType::Str(n) => {
            let s = valor
                .como_str()
                .ok_or_else(|| PhxError::Tipo(format!("esperado Str, recebido {valor:?}")))?;
            let bytes = s.as_bytes();
            if bytes.len() > *n as usize {
                return Err(PhxError::LimiteExcedido(format!(
                    "texto de {} bytes nao cabe na chave Str({n})",
                    bytes.len()
                )));
            }
            corpo[..bytes.len()].copy_from_slice(bytes);
            if nocase {
                for b in corpo.iter_mut() {
                    *b = b.to_ascii_uppercase();
                }
            }
        }
        ColumnType::Bin | ColumnType::Memo => {
            return Err(PhxError::Esquema("Bin/Memo nao entram em indice".into()));
        }
    }

    if desc {
        for b in dst.iter_mut() {
            *b = !*b;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cod(v: Value, ty: ColumnType, desc: bool, nocase: bool) -> Vec<u8> {
        let mut buf = vec![0u8; largura_componente(&ty).unwrap()];
        escrever_componente(&v, &ty, desc, nocase, &mut buf).unwrap();
        buf
    }

    #[test]
    fn inteiros_negativos_ordenam_antes_dos_positivos() {
        let ty = ColumnType::Int4;
        let a = cod(Value::Int(-100), ty, false, false);
        let b = cod(Value::Int(-1), ty, false, false);
        let c = cod(Value::Int(0), ty, false, false);
        let d = cod(Value::Int(100), ty, false, false);
        assert!(a < b && b < c && c < d);
    }

    #[test]
    fn null_vem_primeiro_em_asc_e_ultimo_em_desc() {
        let ty = ColumnType::Int4;
        let nulo = cod(Value::Null, ty, false, false);
        let menor = cod(Value::Int(i32::MIN as i64), ty, false, false);
        assert!(nulo < menor);

        let nulo_d = cod(Value::Null, ty, true, false);
        let maior_d = cod(Value::Int(i32::MAX as i64), ty, true, false);
        assert!(nulo_d > maior_d);
    }

    #[test]
    fn desc_inverte_a_ordem() {
        let ty = ColumnType::Int8;
        let a = cod(Value::Int(1), ty, true, false);
        let b = cod(Value::Int(2), ty, true, false);
        assert!(a > b);
    }

    #[test]
    fn reais_ordenam_incluindo_negativos() {
        let ty = ColumnType::Real8;
        let vals = [-1.0e10, -1.5, -0.0, 0.0, 0.5, 2.5, 1.0e10];
        let mut ant = cod(Value::Real(f64::NEG_INFINITY), ty, false, false);
        for v in vals {
            let atual = cod(Value::Real(v), ty, false, false);
            assert!(ant <= atual, "falhou em {v}");
            ant = atual;
        }
    }

    #[test]
    fn texto_ordena_lexicograficamente() {
        let ty = ColumnType::Str(12);
        let a = cod(Value::Str("Alves".into()), ty, false, false);
        let b = cod(Value::Str("Boller".into()), ty, false, false);
        let c = cod(Value::Str("Costa".into()), ty, false, false);
        assert!(a < b && b < c);
    }

    #[test]
    fn nocase_iguala_maiusculas_e_minusculas() {
        let ty = ColumnType::Str(8);
        assert_eq!(
            cod(Value::Str("boller".into()), ty, false, true),
            cod(Value::Str("BOLLER".into()), ty, false, true)
        );
        assert_ne!(
            cod(Value::Str("boller".into()), ty, false, false),
            cod(Value::Str("BOLLER".into()), ty, false, false)
        );
    }

    #[test]
    fn prefixo_ordena_antes_do_texto_maior() {
        let ty = ColumnType::Str(10);
        let a = cod(Value::Str("Ana".into()), ty, false, false);
        let b = cod(Value::Str("Anabela".into()), ty, false, false);
        assert!(a < b);
    }
}
