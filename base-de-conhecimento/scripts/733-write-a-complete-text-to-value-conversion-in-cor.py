# Write a complete text-to-value conversion in core
# 28/08 19:30

import io
p='crates/phxsql-core/src/carga.rs'
s=io.open(p,encoding='utf-8').read()
i=s.index('// ---------------------------------------------------- do texto para o valor')
j=s.index('#[cfg(test)]\nmod testes {')
novo = '''// ---------------------------------------------------- do texto para o valor
//
// Nos cinco formatos tudo e texto: o `1` de um CSV e a cadeia `"1"`, e nao o
// numero 1. Quem sabe que aquilo e um inteiro e o ESQUEMA, e a conversao tem
// de ser dirigida por ele.
//
// Esta parte mora no nucleo, e nao no servidor, porque a linha de comando
// tambem carrega arquivo -- e duas implementacoes da mesma conversao
// divergiriam no primeiro caso esquisito, que e justamente onde ela e usada.

use crate::datahora::dias_de_civil;
use crate::schema::Schema;
use crate::types::ColumnType;
use crate::uuid::{Uuid, Uuid256};
use crate::value::Value;

/// Decimal exato a partir do texto, ja escalado.
///
/// Texto e nao `f64` de proposito: `f64` nao representa 1,10 exatamente, e
/// dinheiro nao pode perder centavo no caminho.
pub fn texto_para_decimal(texto: &str, escala: u8) -> Result<i128> {
    let t = texto.trim();
    let (negativo, t) = match t.strip_prefix('-') {
        Some(resto) => (true, resto),
        None => (false, t.strip_prefix('+').unwrap_or(t)),
    };
    let invalido = || PhxError::Tipo(format!("decimal invalido: {texto:?}"));
    let (inteiro, fracao) = match t.split_once('.') {
        Some((a, b)) => (a, b),
        None => (t, ""),
    };
    if inteiro.is_empty() && fracao.is_empty() {
        return Err(invalido());
    }
    if !inteiro.chars().all(|c| c.is_ascii_digit()) || !fracao.chars().all(|c| c.is_ascii_digit()) {
        return Err(invalido());
    }
    // Mais casas do que a coluna tem seria perder centavo em silencio.
    if fracao.len() > escala as usize {
        return Err(PhxError::Tipo(format!(
            "{texto:?} tem {} casas decimais e a coluna tem {escala}",
            fracao.len()
        )));
    }
    let mut n: i128 = if inteiro.is_empty() {
        0
    } else {
        inteiro.parse().map_err(|_| invalido())?
    };
    for _ in 0..escala {
        n = n.checked_mul(10).ok_or_else(invalido)?;
    }
    if !fracao.is_empty() {
        let mut f: i128 = fracao.parse().map_err(|_| invalido())?;
        for _ in fracao.len()..escala as usize {
            f *= 10;
        }
        n += f;
    }
    Ok(if negativo { -n } else { n })
}

/// Le uma data em `AAAA-MM-DD` e devolve dias desde a epoca.
pub fn data_de_texto(t: &str) -> Result<i32> {
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

/// Bytes a partir de hexadecimal.
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

/// Normaliza numero escrito a brasileira para a forma que o analisador come.
///
/// # A regra, e o defeito que ela conserta
///
/// Trocar toda virgula por ponto parece obvio e esta errado: `2.000,00` viraria
/// `2.000.00`, que nao e numero nenhum. Aqui o **ultimo separador manda** --
/// ele e o decimal, e o outro e milhar e sai fora.
///
/// | entra | sai |
/// |---|---|
/// | `1500,50` | `1500.50` |
/// | `1.500,50` | `1500.50` |
/// | `1,500.50` | `1500.50` |
/// | `1500.50` | `1500.50` |
/// | `1.500` | `1.500` -- ambiguo, e fica como esta |
///
/// A ultima linha e a decisao dificil: `1.500` pode ser mil e quinhentos ou um
/// e meio, e nao ha como saber. Fica como veio, e o tipo da coluna decide.
/// Adivinhar mudaria o valor de quem digitou certo.
pub fn numero_pt(t: &str) -> String {
    let ponto = t.rfind('.');
    let virgula = t.rfind(',');
    match (ponto, virgula) {
        (Some(p), Some(v)) if v > p => t.replace('.', "").replace(',', "."),
        (Some(p), Some(v)) if p > v => t.replace(',', ""),
        (None, Some(_)) => t.replace(',', "."),
        _ => t.to_string(),
    }
}

/// Converte um texto para o valor da coluna, dirigido pelo tipo dela.
///
/// **Campo vazio vira NULO**, e nao zero nem cadeia vazia: numa planilha a
/// celula em branco quer dizer «nao informado», e gravar zero num campo de
/// valor mudaria o dado.
pub fn valor_de_texto(t: &str, ty: &ColumnType) -> Result<Value> {
    let t = t.trim();
    if t.is_empty() {
        return Ok(Value::Null);
    }
    let erro = |esperado: &str| PhxError::Tipo(format!("esperado {esperado}, recebido {t:?}"));

    Ok(match ty {
        ColumnType::Bool => match t.to_ascii_lowercase().as_str() {
            "1" | "true" | "sim" | "s" | "verdadeiro" | "v" | "yes" | "y" => Value::Bool(true),
            "0" | "false" | "nao" | "não" | "n" | "falso" | "no" => Value::Bool(false),
            _ => return Err(erro("sim/nao, true/false ou 1/0")),
        },
        ColumnType::Int1 | ColumnType::Int2 | ColumnType::Int4 | ColumnType::Int8 => {
            Value::Int(t.parse::<i64>().map_err(|_| erro("inteiro"))?)
        }
        ColumnType::UInt1
        | ColumnType::UInt2
        | ColumnType::UInt4
        | ColumnType::UInt8
        | ColumnType::Sequence => {
            Value::UInt(t.parse::<u64>().map_err(|_| erro("inteiro sem sinal"))?)
        }
        ColumnType::Real4 | ColumnType::Real8 => {
            Value::Real(numero_pt(t).parse::<f64>().map_err(|_| erro("numero"))?)
        }
        ColumnType::Decimal { escala, .. } => Value::Decimal(texto_para_decimal(&numero_pt(t), *escala)?),
        ColumnType::Date => Value::Date(data_de_texto(t)?),
        // Hora e instante chegam em numero -- centesimos desde a meia-noite e
        // milissegundos desde a epoca. Texto de relogio (`14:30`) nao entra
        // ainda, e o erro diz o que se espera em vez de gravar zero.
        ColumnType::Time => Value::Time(t.parse::<i32>().map_err(|_| erro("hora em centesimos"))?),
        ColumnType::DateTime => {
            Value::DateTime(t.parse::<i64>().map_err(|_| erro("instante em milissegundos"))?)
        }
        ColumnType::Uuid if t.eq_ignore_ascii_case("novo") || t.eq_ignore_ascii_case("v7") => {
            Value::Uuid(Uuid::v7())
        }
        ColumnType::Uuid if t.eq_ignore_ascii_case("v4") => Value::Uuid(Uuid::v4()),
        ColumnType::Uuid => Value::Uuid(Uuid::de_texto(t)?),
        ColumnType::Uuid256 if t.eq_ignore_ascii_case("novo") => {
            Value::Uuid256(Uuid256::aleatorio())
        }
        ColumnType::Uuid256 => Value::Uuid256(Uuid256::de_texto(t)?),
        ColumnType::Str(_) => Value::Str(t.to_string()),
        ColumnType::Memo => Value::Memo(t.to_string()),
        ColumnType::Bin => Value::Bin(hex_para_bytes(t)?),
    })
}

/// Uma linha da carga virada em valores, casando as colunas POR NOME.
///
/// Por nome, e nao por posicao: uma coluna a mais no meio do arquivo gravaria
/// tudo deslocado -- sem erro, porque os tipos costumam aceitar.
///
/// Coluna do arquivo que a tabela nao tem e ERRO, com o nome dela. Coluna da
/// tabela que o arquivo nao traz fica nula -- ou com o padrao, no caso das
/// colunas de sistema.
pub fn linha_de_texto(carga: &Carga, i: usize, esquema: &Schema) -> Result<Vec<Value>> {
    let linha = carga
        .linhas
        .get(i)
        .ok_or_else(|| PhxError::Tipo(format!("a carga nao tem a linha {}", i + 1)))?;
    for c in &carga.colunas {
        if esquema.coluna_por_nome(c).is_none() {
            return Err(PhxError::Tipo(format!(
                "coluna {c:?} nao existe em {}",
                esquema.nome()
            )));
        }
    }
    esquema
        .colunas()
        .iter()
        .map(|col| match carga.colunas.iter().position(|c| *c == col.nome) {
            Some(j) => valor_de_texto(linha.get(j).map(String::as_str).unwrap_or(""), &col.ty),
            None if col.nome == crate::schema::COLUNA_SOFTDELETED => Ok(Value::Bool(false)),
            None if col.nome == crate::schema::COLUNA_ROWNUM => Ok(Value::UInt(0)),
            None => Ok(Value::Null),
        })
        .collect()
}

'''
s = s[:i] + novo + s[j:]
io.open(p,'w',encoding='utf-8').write(s)
