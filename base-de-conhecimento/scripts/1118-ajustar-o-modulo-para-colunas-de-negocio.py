# Ajustar o modulo para colunas de negocio
# 29/08 11:37

import io
p='crates/phxsql-server/src/dblink/sincronia.rs'
s=io.open(p,encoding='utf-8').read()

velho='''/// Converte uma linha remota (texto do fio) para os `Value` do esquema local.
///
/// `mapa` diz onde cada coluna remota mora na linha local; as colunas de
/// sistema locais (`softdeleted`, `rownum`) ficam `Null` e o motor as preenche.
pub fn linha_remota_para_local(
    esquema: &Schema,
    mapa: &[(usize, usize)],
    remota: &[Option<String>],
) -> Result<Vec<Value>> {
    let mut linha = vec![Value::Null; esquema.colunas().len()];
    for (de, para) in mapa {
        let ty = &esquema.colunas()[*para].ty;
        linha[*para] = match &remota[*de] {
            None => Value::Null,
            Some(t) => valor_de_texto(t, ty)?,
        };
    }
    Ok(linha)
}'''
novo='''/// As posicoes das colunas de NEGOCIO do esquema local -- tudo menos as de
/// sistema (`softdeleted`, `rownum`), que o motor preenche sozinho.
///
/// A sincronia so fala destas: e o que deixa `inserir`/`atualizar` receberem a
/// linha sem as colunas do motor, como qualquer cliente.
pub fn posicoes_de_negocio(esquema: &Schema) -> Vec<usize> {
    esquema
        .colunas()
        .iter()
        .enumerate()
        .filter(|(_, c)| c.nome != "softdeleted" && c.nome != "rownum")
        .map(|(i, _)| i)
        .collect()
}

/// Converte uma linha remota (texto do fio) para a linha de NEGOCIO local.
///
/// Toda coluna de negocio local precisa vir do outro lado -- faltar uma seria
/// inserir com buraco silencioso, entao e recusa com o conserto no texto.
pub fn linha_remota_para_negocio(
    esquema: &Schema,
    negocio: &[usize],
    mapa: &[(usize, usize)],
    remota: &[Option<String>],
) -> Result<Vec<Value>> {
    let mut linha = Vec::with_capacity(negocio.len());
    for pos in negocio {
        let de = mapa.iter().find(|(_, para)| para == pos).map(|(de, _)| *de);
        let Some(de) = de else {
            return Err(PhxError::Esquema(format!(
                "a coluna local {:?} nao existe na tabela remota: rode o \\
                 assistente de novo para recriar a ligacao",
                esquema.colunas()[*pos].nome
            )));
        };
        let ty = &esquema.colunas()[*pos].ty;
        linha.push(match &remota[de] {
            None => Value::Null,
            Some(t) => valor_de_texto(t, ty)?,
        });
    }
    Ok(linha)
}'''
assert s.count(velho)==1
s=s.replace(velho,novo)

# plano sem o corte por prefixo: as linhas JA sao de negocio
velho2='''pub fn plano(
    sentido: Sentido,
    dono: Dono,
    remotas: &HashMap<String, Vec<Value>>,
    locais: &HashMap<String, Vec<Value>>,
    colunas_de_negocio: usize,
) -> Plano {
    let mut p = Plano::default();
    let corta = |l: &Vec<Value>| l[..colunas_de_negocio.min(l.len())].to_vec();

    for (chave, r) in remotas {
        match locais.get(chave) {
            None => {
                if sentido != Sentido::Empurrar {
                    p.para_ca.push(r.clone());
                }
            }
            Some(l) if corta(l) == corta(r) => p.iguais += 1,
            Some(l) => {
                p.conflitos += 1;
                match sentido {
                    Sentido::Puxar => p.para_ca.push(r.clone()),
                    Sentido::Empurrar => p.para_la.push(corta(l)),
                    Sentido::Dois => match dono {
                        Dono::Aqui => p.para_la.push(corta(l)),
                        Dono::La => p.para_ca.push(r.clone()),
                    },
                }
            }
        }
    }
    for (chave, l) in locais {
        if !remotas.contains_key(chave) && sentido != Sentido::Puxar {
            p.para_la.push(corta(l));
        }
    }
    p
}'''
novo2='''pub fn plano(
    sentido: Sentido,
    dono: Dono,
    remotas: &HashMap<String, Vec<Value>>,
    locais: &HashMap<String, Vec<Value>>,
) -> Plano {
    let mut p = Plano::default();
    for (chave, r) in remotas {
        match locais.get(chave) {
            None => {
                if sentido != Sentido::Empurrar {
                    p.para_ca.push(r.clone());
                }
            }
            Some(l) if l == r => p.iguais += 1,
            Some(l) => {
                p.conflitos += 1;
                match sentido {
                    Sentido::Puxar => p.para_ca.push(r.clone()),
                    Sentido::Empurrar => p.para_la.push(l.clone()),
                    Sentido::Dois => match dono {
                        Dono::Aqui => p.para_la.push(l.clone()),
                        Dono::La => p.para_ca.push(r.clone()),
                    },
                }
            }
        }
    }
    for (chave, l) in locais {
        if !remotas.contains_key(chave) && sentido != Sentido::Puxar {
            p.para_la.push(l.clone());
        }
    }
    p
}'''
assert s.count(velho2)==1
s=s.replace(velho2,novo2)

# o empurrao passa a receber nomes e tipos das colunas de negocio
velho3='''pub fn sql_do_empurrao(
    tabela_remota: &str,
    esquema: &Schema,
    mapa: &[(usize, usize)],
    linhas: &[Vec<Value>],
    por_lote: usize,
) -> Result<Vec<String>> {
    if linhas.is_empty() {
        return Ok(Vec::new());
    }
    let nomes: Vec<String> = mapa
        .iter()
        .map(|(_, p)| entre_crases(&esquema.colunas()[*p].nome))
        .collect();
    let atualiza: Vec<String> = nomes.iter().map(|n| format!("{n}=VALUES({n})")).collect();

    let mut sqls = Vec::new();
    for lote in linhas.chunks(por_lote.max(1)) {
        let mut valores = Vec::with_capacity(lote.len());
        for l in lote {
            let mut celulas = Vec::with_capacity(mapa.len());
            for (_, p) in mapa {
                celulas.push(valor_para_sql(&l[*p], &esquema.colunas()[*p].ty)?);
            }
            valores.push(format!("({})", celulas.join(",")));
        }
        sqls.push(format!(
            "INSERT INTO {} ({}) VALUES {} ON DUPLICATE KEY UPDATE {}",
            entre_crases(tabela_remota),
            nomes.join(","),
            valores.join(","),
            atualiza.join(",")
        ));
    }
    Ok(sqls)
}'''
novo3='''pub fn sql_do_empurrao(
    tabela_remota: &str,
    colunas: &[(String, ColumnType)],
    linhas: &[Vec<Value>],
    por_lote: usize,
) -> Result<Vec<String>> {
    if linhas.is_empty() {
        return Ok(Vec::new());
    }
    let nomes: Vec<String> = colunas.iter().map(|(n, _)| entre_crases(n)).collect();
    let atualiza: Vec<String> = nomes.iter().map(|n| format!("{n}=VALUES({n})")).collect();

    let mut sqls = Vec::new();
    for lote in linhas.chunks(por_lote.max(1)) {
        let mut valores = Vec::with_capacity(lote.len());
        for l in lote {
            let mut celulas = Vec::with_capacity(colunas.len());
            for (i, (_, ty)) in colunas.iter().enumerate() {
                celulas.push(valor_para_sql(&l[i], ty)?);
            }
            valores.push(format!("({})", celulas.join(",")));
        }
        sqls.push(format!(
            "INSERT INTO {} ({}) VALUES {} ON DUPLICATE KEY UPDATE {}",
            entre_crases(tabela_remota),
            nomes.join(","),
            valores.join(","),
            atualiza.join(",")
        ));
    }
    Ok(sqls)
}'''
assert s.count(velho3)==1
s=s.replace(velho3,novo3)
io.open(p,'w',encoding='utf-8').write(s)
print('sincronia ajustada')
