# Make json_para_linha fill the system column
# 28/08 17:35

import io
p='crates/phxsql-server/src/valores.rs'
s=io.open(p,encoding='utf-8').read()
velho='''pub fn json_para_linha(j: &Json, esquema: &Schema) -> Result<Vec<Value>> {
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
}'''
novo='''pub fn json_para_linha(j: &Json, esquema: &Schema) -> Result<Vec<Value>> {
    let colunas = esquema.colunas();
    // A coluna de sistema pode ficar de fora do que chega pela rede: quem
    // manda a linha declarou as colunas dele e nao tem por que saber dela.
    // Falta ela na lista -> entra `false` no fim; falta no objeto -> idem.
    // Sem isso, `inserir` recusaria toda linha de todo cliente que existe
    // hoje, porque a coluna e obrigatoria e o ausente vira nulo.
    let sistema = esquema.coluna_softdeleted();
    let padrao_de = |i: usize| -> Value {
        if Some(i) == sistema {
            Value::Bool(false)
        } else {
            Value::Null
        }
    };
    match j {
        Json::Lista(itens) => {
            let curta = sistema.is_some_and(|i| itens.len() == i);
            if itens.len() != colunas.len() && !curta {
                return Err(PhxError::Tipo(format!(
                    "a lista tem {} valores, a tabela tem {} colunas",
                    itens.len(),
                    colunas.len()
                )));
            }
            colunas
                .iter()
                .enumerate()
                .map(|(i, c)| match itens.get(i) {
                    Some(v) => json_para_valor(v, &c.ty),
                    None => Ok(padrao_de(i)),
                })
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
                .enumerate()
                .map(|(i, c)| match j.campo(&c.nome) {
                    Some(v) => json_para_valor(v, &c.ty),
                    None => Ok(padrao_de(i)),
                })
                .collect()
        }
        _ => Err(PhxError::Tipo(
            "a linha precisa ser um objeto ou uma lista".into(),
        )),
    }
}'''
assert velho in s
s=s.replace(velho,novo,1)
io.open(p,'w',encoding='utf-8').write(s)
