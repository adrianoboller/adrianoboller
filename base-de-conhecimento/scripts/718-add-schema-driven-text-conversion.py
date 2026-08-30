# Add schema-driven text conversion
# 28/08 19:23

import io
p='crates/phxsql-server/src/valores.rs'
s=io.open(p,encoding='utf-8').read()
velho='''pub fn json_para_valor(j: &Json, ty: &ColumnType) -> Result<Value> {'''
novo='''/// Converte um valor que veio de um formato de TEXTO -- CSV, TXT, HTML, XML.
///
/// # Por que existe separado
///
/// Nesses formatos tudo e texto: o `1` de um CSV e a cadeia `"1"`, e nao o
/// numero 1. Quem sabe que aquilo e um inteiro e o ESQUEMA, e a conversao tem
/// de ser dirigida por ele.
///
/// Nao virou leniencia do `json_para_valor` de proposito. Ali a rigidez pega
/// defeito de cliente: quem manda `{"id":"1"}` num JSON provavelmente errou o
/// tipo no codigo dele, e engolir calado esconderia isso. Aqui a origem e
/// texto por definicao, e recusar seria recusar o formato inteiro.
///
/// Campo vazio vira NULO, e nao zero nem cadeia vazia: numa planilha a celula
/// em branco quer dizer "nao informado", e gravar zero num campo de valor
/// mudaria o dado.
pub fn json_para_valor_de_texto(j: &Json, ty: &ColumnType) -> Result<Value> {
    let Json::Texto(t) = j else {
        // Ja veio tipado (o caso do JSON): segue o caminho normal.
        return json_para_valor(j, ty);
    };
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
        ColumnType::UInt1 | ColumnType::UInt2 | ColumnType::UInt4 | ColumnType::UInt8 => {
            Value::UInt(t.parse::<u64>().map_err(|_| erro("inteiro sem sinal"))?)
        }
        ColumnType::Real4 | ColumnType::Real8 => {
            // Virgula decimal tambem entra: e o que sai de uma planilha em
            // portugues, e recusar obrigaria a editar o arquivo antes de colar.
            let limpo = t.replace(',', ".");
            Value::Real(limpo.parse::<f64>().map_err(|_| erro("numero"))?)
        }
        ColumnType::Decimal { escala, .. } => {
            Value::Decimal(texto_para_decimal(&t.replace(',', "."), *escala)?)
        }
        // Os demais ja aceitam texto no caminho normal: data, hora, uuid,
        // string, memo. Reescrever a analise deles aqui seria ter duas.
        _ => json_para_valor(&Json::texto_de(t), ty)?,
    })
}

/// Uma linha inteira vinda de formato de texto. Ver [`json_para_valor_de_texto`].
pub fn json_para_linha_de_texto(j: &Json, esquema: &Schema) -> Result<Vec<Value>> {
    let Json::Objeto(pares) = j else {
        return Err(PhxError::Tipo("a linha precisa ser um objeto".into()));
    };
    for (chave, _) in pares {
        if esquema.coluna_por_nome(chave).is_none() {
            return Err(PhxError::Tipo(format!(
                "coluna {chave:?} nao existe em {}",
                esquema.nome()
            )));
        }
    }
    let sistema = esquema.coluna_softdeleted();
    esquema
        .colunas()
        .iter()
        .enumerate()
        .map(|(i, c)| match j.campo(&c.nome) {
            Some(v) => json_para_valor_de_texto(v, &c.ty),
            None if Some(i) == sistema => Ok(Value::Bool(false)),
            None if c.nome == phxsql_core::schema::COLUNA_ROWNUM => Ok(Value::UInt(0)),
            None => Ok(Value::Null),
        })
        .collect()
}

pub fn json_para_valor(j: &Json, ty: &ColumnType) -> Result<Value> {'''
assert velho in s
s=s.replace(velho,novo,1)
io.open(p,'w',encoding='utf-8').write(s)
