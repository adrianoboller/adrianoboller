# Fix Nagle and mark rownum as a system column
# 28/08 18:39

import io
p='crates/phxsql-server/src/valores.rs'
s=io.open(p,encoding='utf-8').read()
velho='''pub fn colunas_para_json(esquema: &Schema) -> Json {
    let sistema = esquema.coluna_softdeleted();
    Json::Lista(
        esquema
            .colunas()
            .iter()
            .enumerate()
            .map(|(i, c)| {
                Json::objeto(vec![
                    ("nome", Json::texto_de(&c.nome)),
                    ("rotulo", Json::texto_de(c.rotulo())),
                    ("tipo", Json::texto_de(format!("{:?}", c.ty))),
                    ("nullable", Json::Bool(c.nullable)),
                    ("sistema", Json::Bool(Some(i) == sistema)),
                ])
            })
            .collect(),
    )
}'''
novo='''pub fn colunas_para_json(esquema: &Schema) -> Json {
    Json::Lista(
        esquema
            .colunas()
            .iter()
            .map(|c| {
                Json::objeto(vec![
                    ("nome", Json::texto_de(&c.nome)),
                    ("rotulo", Json::texto_de(c.rotulo())),
                    ("tipo", Json::texto_de(format!("{:?}", c.ty))),
                    ("nullable", Json::Bool(c.nullable)),
                    (
                        "sistema",
                        Json::Bool(phxsql_core::schema::e_coluna_de_sistema(&c.nome)),
                    ),
                ])
            })
            .collect(),
    )
}'''
assert velho in s
s=s.replace(velho,novo,1)
io.open(p,'w',encoding='utf-8').write(s)
