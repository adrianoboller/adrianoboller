//! Traducao entre o JSON do protocolo e os valores do PhxSql.
//!
//! O tipo da coluna e quem manda: o mesmo `42` vira `Int`, `Date` ou
//! `Decimal` conforme a coluna que o recebe.
//!
//! Duas escolhas que valem explicacao:
//!
//! * **Decimal sai como texto**, nunca como numero JSON. `f64` nao representa
//!   1.10 exatamente, e um campo de dinheiro nao pode perder centavo no
//!   caminho de ida e volta.
//! * **Binario sai como hexadecimal**, que atravessa JSON sem escape e e
//!   conferivel a olho nu.

use phxsql_core::datahora::{data_iso, dias_de_civil, hora_iso};
use phxsql_core::error::{PhxError, Result};
use phxsql_core::json::Json;
use phxsql_core::schema::Schema;
use phxsql_core::types::ColumnType;
use phxsql_core::value::Value;

/// Formata um decimal escalado como texto: 1234 com escala 2 vira "12.34".
pub fn decimal_para_texto(valor: i128, escala: u8) -> String {
    if escala == 0 {
        return valor.to_string();
    }
    let divisor = 10i128.pow(escala as u32);
    let sinal = if valor < 0 { "-" } else { "" };
    let a = valor.unsigned_abs();
    let d = divisor.unsigned_abs();
    format!(
        "{sinal}{}.{:0>largura$}",
        a / d,
        a % d,
        largura = escala as usize
    )
}

/// Le "12.34" com escala 2 e devolve 1234.
pub fn texto_para_decimal(texto: &str, escala: u8) -> Result<i128> {
    let t = texto.trim();
    let (negativo, t) = match t.strip_prefix('-') {
        Some(resto) => (true, resto),
        None => (false, t.strip_prefix('+').unwrap_or(t)),
    };
    let (inteiro, fracao) = match t.split_once('.') {
        Some((i, f)) => (i, f),
        None => (t, ""),
    };
    if inteiro.is_empty() && fracao.is_empty() {
        return Err(PhxError::Tipo(format!("decimal invalido: {texto:?}")));
    }
    if !inteiro.chars().all(|c| c.is_ascii_digit()) || !fracao.chars().all(|c| c.is_ascii_digit()) {
        return Err(PhxError::Tipo(format!("decimal invalido: {texto:?}")));
    }
    if fracao.len() > escala as usize {
        return Err(PhxError::LimiteExcedido(format!(
            "{texto:?} tem {} casas decimais, a coluna aceita {escala}",
            fracao.len()
        )));
    }
    let mut digitos = String::from(if inteiro.is_empty() { "0" } else { inteiro });
    digitos.push_str(fracao);
    for _ in fracao.len()..escala as usize {
        digitos.push('0');
    }
    let n: i128 = digitos
        .parse()
        .map_err(|_| PhxError::LimiteExcedido(format!("decimal fora de faixa: {texto:?}")))?;
    Ok(if negativo { -n } else { n })
}

pub fn bytes_para_hex(b: &[u8]) -> String {
    let mut s = String::with_capacity(b.len() * 2);
    for byte in b {
        s.push_str(&format!("{byte:02x}"));
    }
    s
}

pub fn hex_para_bytes(hex: &str) -> Result<Vec<u8>> {
    let t = hex.trim();
    if t.len() % 2 != 0 {
        return Err(PhxError::Tipo(
            "hexadecimal precisa ter quantidade par de digitos".into(),
        ));
    }
    (0..t.len())
        .step_by(2)
        .map(|i| {
            u8::from_str_radix(&t[i..i + 2], 16)
                .map_err(|_| PhxError::Tipo(format!("hexadecimal invalido: {hex:?}")))
        })
        .collect()
}

/// Le uma data em `AAAA-MM-DD` e devolve dias desde a epoca.
fn data_de_texto(t: &str) -> Result<i32> {
    let partes: Vec<&str> = t.trim().split('-').collect();
    let invalida = || PhxError::Tipo(format!("data invalida: {t:?} (use AAAA-MM-DD)"));
    if partes.len() != 3 {
        return Err(invalida());
    }
    let ano: i32 = partes[0].parse().map_err(|_| invalida())?;
    let mes: u32 = partes[1].parse().map_err(|_| invalida())?;
    let dia: u32 = partes[2].parse().map_err(|_| invalida())?;
    if !(1..=12).contains(&mes) || !(1..=31).contains(&dia) {
        return Err(invalida());
    }
    Ok(dias_de_civil(ano, mes, dia))
}

/// Valor do PhxSql em JSON, usando o tipo da coluna para dar sentido aos
/// inteiros de data, hora e decimal.
pub fn valor_para_json(v: &Value, ty: &ColumnType) -> Json {
    match (v, ty) {
        (Value::Null, _) => Json::Nulo,
        (Value::Bool(b), _) => Json::Bool(*b),
        (Value::Date(d), _) => Json::texto_de(data_iso(*d)),
        (Value::Time(c), _) => Json::texto_de(hora_iso(*c)),
        (Value::DateTime(ms), _) => Json::texto_de(phxsql_core::datahora::instante_iso(*ms)),
        (Value::Decimal(n), ColumnType::Decimal { escala, .. }) => {
            Json::texto_de(decimal_para_texto(*n, *escala))
        }
        (Value::Decimal(n), _) => Json::texto_de(n.to_string()),
        (Value::Int(n), _) => Json::de_i64(*n),
        (Value::UInt(n), _) => Json::de_u64(*n),
        (Value::Real(n), _) => Json::Numero(*n),
        (Value::Str(s), _) | (Value::Memo(s), _) => Json::texto_de(s),
        (Value::Bin(b), _) => Json::texto_de(bytes_para_hex(b)),
    }
}

/// JSON em valor do PhxSql, guiado pelo tipo da coluna.
pub fn json_para_valor(j: &Json, ty: &ColumnType) -> Result<Value> {
    if j.e_nulo() {
        return Ok(Value::Null);
    }
    let erro = |esperado: &str| PhxError::Tipo(format!("esperado {esperado}, recebido {j:?}"));

    Ok(match ty {
        ColumnType::Bool => match j {
            Json::Bool(b) => Value::Bool(*b),
            Json::Numero(n) => Value::Bool(*n != 0.0),
            _ => return Err(erro("booleano")),
        },
        ColumnType::Int1 | ColumnType::Int2 | ColumnType::Int4 | ColumnType::Int8 => {
            Value::Int(j.inteiro().ok_or_else(|| erro("inteiro"))?)
        }
        ColumnType::UInt1 | ColumnType::UInt2 | ColumnType::UInt4 | ColumnType::UInt8 => {
            let n = j.inteiro().ok_or_else(|| erro("inteiro sem sinal"))?;
            if n < 0 {
                return Err(PhxError::Tipo(format!(
                    "{n} e negativo numa coluna sem sinal"
                )));
            }
            Value::UInt(n as u64)
        }
        ColumnType::Real4 | ColumnType::Real8 => {
            Value::Real(j.numero().ok_or_else(|| erro("numero"))?)
        }
        ColumnType::Decimal { escala, .. } => {
            match j {
                Json::Texto(t) => Value::Decimal(texto_para_decimal(t, *escala)?),
                Json::Numero(_) => return Err(PhxError::Tipo(
                    "decimal precisa vir como texto (\"12.34\"), para nao perder centavo em f64"
                        .into(),
                )),
                _ => return Err(erro("decimal em texto")),
            }
        }
        ColumnType::Date => match j {
            Json::Texto(t) => Value::Date(data_de_texto(t)?),
            Json::Numero(_) => Value::Date(j.inteiro().ok_or_else(|| erro("data"))? as i32),
            _ => return Err(erro("data")),
        },
        ColumnType::Time => Value::Time(j.inteiro().ok_or_else(|| erro("hora"))? as i32),
        ColumnType::DateTime => Value::DateTime(j.inteiro().ok_or_else(|| erro("data e hora"))?),
        ColumnType::Str(_) => Value::Str(j.texto().ok_or_else(|| erro("texto"))?.to_string()),
        ColumnType::Memo => Value::Memo(j.texto().ok_or_else(|| erro("texto"))?.to_string()),
        ColumnType::Bin => match j {
            Json::Texto(t) => Value::Bin(hex_para_bytes(t)?),
            Json::Lista(l) => Value::Bin(
                l.iter()
                    .map(|x| {
                        x.inteiro()
                            .filter(|n| (0..=255).contains(n))
                            .map(|n| n as u8)
                            .ok_or_else(|| PhxError::Tipo("byte fora de 0..255".into()))
                    })
                    .collect::<Result<Vec<u8>>>()?,
            ),
            _ => return Err(erro("binario em hexadecimal")),
        },
    })
}

/// Linha inteira em JSON, como objeto com o nome de cada coluna.
pub fn linha_para_json(linha: &[Value], esquema: &Schema) -> Json {
    Json::Objeto(
        esquema
            .colunas()
            .iter()
            .zip(linha.iter())
            .map(|(c, v)| (c.nome.clone(), valor_para_json(v, &c.ty)))
            .collect(),
    )
}

/// Aceita a linha como objeto (por nome de coluna) ou como lista (na ordem do
/// esquema). Colunas ausentes no objeto entram como NULL.
pub fn json_para_linha(j: &Json, esquema: &Schema) -> Result<Vec<Value>> {
    let colunas = esquema.colunas();
    match j {
        Json::Lista(itens) => {
            if itens.len() != colunas.len() {
                return Err(PhxError::Tipo(format!(
                    "a lista tem {} valores, a tabela tem {} colunas",
                    itens.len(),
                    colunas.len()
                )));
            }
            itens
                .iter()
                .zip(colunas.iter())
                .map(|(v, c)| json_para_valor(v, &c.ty))
                .collect()
        }
        Json::Objeto(pares) => {
            for (chave, _) in pares {
                if esquema.coluna_por_nome(chave).is_none() {
                    return Err(PhxError::Tipo(format!(
                        "coluna {chave:?} nao existe em {}",
                        esquema.nome()
                    )));
                }
            }
            colunas
                .iter()
                .map(|c| match j.campo(&c.nome) {
                    Some(v) => json_para_valor(v, &c.ty),
                    None => Ok(Value::Null),
                })
                .collect()
        }
        _ => Err(PhxError::Tipo(
            "a linha precisa ser um objeto ou uma lista".into(),
        )),
    }
}

/// Chave de indice: lista de valores na ordem das colunas do indice.
pub fn json_para_chave(j: &Json, esquema: &Schema, indice: usize) -> Result<Vec<Value>> {
    let def = esquema
        .indices()
        .get(indice)
        .ok_or_else(|| PhxError::NaoEncontrado(format!("indice {indice} inexistente")))?;
    let itens = match j {
        Json::Lista(l) => l.clone(),
        outro => vec![outro.clone()],
    };
    if itens.len() != def.colunas.len() {
        return Err(PhxError::Tipo(format!(
            "o indice {} tem {} colunas, a chave veio com {}",
            def.nome,
            def.colunas.len(),
            itens.len()
        )));
    }
    itens
        .iter()
        .zip(def.colunas.iter())
        .map(|(v, ic)| json_para_valor(v, &esquema.colunas()[ic.coluna].ty))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decimal_vai_e_volta_sem_perder_centavo() {
        for (valor, escala, texto) in [
            (150_000i128, 2u8, "1500.00"),
            (-1_234, 2, "-12.34"),
            (5, 2, "0.05"),
            (0, 2, "0.00"),
            (42, 0, "42"),
            (999_999_999_999_999, 4, "99999999999.9999"),
        ] {
            assert_eq!(decimal_para_texto(valor, escala), texto, "escala {escala}");
            assert_eq!(texto_para_decimal(texto, escala).unwrap(), valor);
        }
    }

    #[test]
    fn decimal_recusa_numero_json() {
        let ty = ColumnType::Decimal {
            precisao: 15,
            escala: 2,
        };
        let e = json_para_valor(&Json::Numero(12.34), &ty).unwrap_err();
        assert!(format!("{e}").contains("texto"), "erro foi {e}");
        assert_eq!(
            json_para_valor(&Json::texto_de("12.34"), &ty).unwrap(),
            Value::Decimal(1_234)
        );
    }

    #[test]
    fn decimal_recusa_casas_demais() {
        assert!(texto_para_decimal("1.234", 2).is_err());
        assert!(texto_para_decimal("abc", 2).is_err());
        assert!(texto_para_decimal("", 2).is_err());
        assert_eq!(texto_para_decimal("1.2", 2).unwrap(), 120);
        assert_eq!(texto_para_decimal("+7", 2).unwrap(), 700);
    }

    #[test]
    fn hexadecimal_vai_e_volta() {
        let b = vec![0u8, 1, 15, 16, 254, 255];
        let h = bytes_para_hex(&b);
        assert_eq!(h, "00010f10feff");
        assert_eq!(hex_para_bytes(&h).unwrap(), b);
        assert!(hex_para_bytes("abc").is_err());
        assert!(hex_para_bytes("zz").is_err());
    }

    #[test]
    fn data_aceita_iso_e_numero() {
        let ty = ColumnType::Date;
        assert_eq!(
            json_para_valor(&Json::texto_de("2024-10-04"), &ty).unwrap(),
            Value::Date(20_000)
        );
        assert_eq!(
            json_para_valor(&Json::Numero(20_000.0), &ty).unwrap(),
            Value::Date(20_000)
        );
        assert_eq!(
            valor_para_json(&Value::Date(20_000), &ty),
            Json::texto_de("2024-10-04")
        );
        assert!(json_para_valor(&Json::texto_de("04/10/2024"), &ty).is_err());
        assert!(json_para_valor(&Json::texto_de("2024-13-01"), &ty).is_err());
    }

    fn esquema() -> Schema {
        use phxsql_core::schema::{Column, IndexColumn, IndexDef};
        Schema::new(
            "clientes",
            vec![
                Column::new("id", ColumnType::Int8).obrigatoria(),
                Column::new("nome", ColumnType::Str(40)),
                Column::new(
                    "limite",
                    ColumnType::Decimal {
                        precisao: 15,
                        escala: 2,
                    },
                ),
            ],
            vec![IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico()],
        )
        .unwrap()
    }

    #[test]
    fn linha_como_objeto_ou_lista() {
        let esq = esquema();
        let por_objeto = Json::analisar(r#"{"id":7,"nome":"Ana","limite":"10.50"}"#).unwrap();
        let por_lista = Json::analisar(r#"[7,"Ana","10.50"]"#).unwrap();
        let a = json_para_linha(&por_objeto, &esq).unwrap();
        let b = json_para_linha(&por_lista, &esq).unwrap();
        assert_eq!(a, b);
        assert_eq!(a[0], Value::Int(7));
        assert_eq!(a[2], Value::Decimal(1_050));

        // A volta preserva os nomes e o decimal em texto.
        let volta = linha_para_json(&a, &esq);
        assert_eq!(volta.texto_ou("nome", ""), "Ana");
        assert_eq!(volta.campo("limite").unwrap().texto().unwrap(), "10.50");
    }

    #[test]
    fn coluna_ausente_no_objeto_vira_null() {
        let esq = esquema();
        let j = Json::analisar(r#"{"id":1}"#).unwrap();
        let linha = json_para_linha(&j, &esq).unwrap();
        assert_eq!(linha[1], Value::Null);
        assert_eq!(linha[2], Value::Null);
    }

    #[test]
    fn coluna_inventada_e_recusada() {
        let esq = esquema();
        let j = Json::analisar(r#"{"id":1,"inexistente":2}"#).unwrap();
        assert!(json_para_linha(&j, &esq).is_err());
    }

    #[test]
    fn lista_com_tamanho_errado_e_recusada() {
        let esq = esquema();
        assert!(json_para_linha(&Json::analisar("[1,2]").unwrap(), &esq).is_err());
    }

    #[test]
    fn chave_de_indice_aceita_escalar_e_lista() {
        let esq = esquema();
        let a = json_para_chave(&Json::Numero(7.0), &esq, 0).unwrap();
        let b = json_para_chave(&Json::analisar("[7]").unwrap(), &esq, 0).unwrap();
        assert_eq!(a, b);
        assert_eq!(a[0], Value::Int(7));
        assert!(json_para_chave(&Json::analisar("[7,8]").unwrap(), &esq, 0).is_err());
    }
}
