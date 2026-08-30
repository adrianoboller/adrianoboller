# Enrich the esquema response
# 28/08 11:22

import pathlib
p = pathlib.Path('crates/phxsql-server/src/servidor.rs')
s = p.read_text()

v = '''        let colunas: Vec<Json> = e
            .colunas()
            .iter()
            .map(|c| {
                Json::objeto(vec![
                    ("nome", Json::texto_de(&c.nome)),
                    ("tipo", Json::texto_de(format!("{:?}", c.ty))),
                    ("nullable", Json::Bool(c.nullable)),
                ])
            })
            .collect();'''
n = '''        let colunas: Vec<Json> = e
            .colunas()
            .iter()
            .enumerate()
            .map(|(i, c)| {
                let papel = e.papel_da_coluna(i);
                Json::objeto(vec![
                    ("id", Json::texto_de(c.id.to_string())),
                    ("nome", Json::texto_de(&c.nome)),
                    ("caption", Json::texto_de(&c.caption)),
                    ("rotulo", Json::texto_de(c.rotulo())),
                    ("descricao", Json::texto_de(&c.descricao)),
                    ("mascara", Json::texto_de(&c.mascara)),
                    ("tipo", Json::texto_de(format!("{:?}", c.ty))),
                    ("tamanho", Json::de_u64(largura_do_tipo(&c.ty))),
                    ("nullable", Json::Bool(c.nullable)),
                    // O papel nas chaves e DERIVADO dos indices e das FKs, e
                    // por isso nao pode discordar delas.
                    ("primaria", Json::Bool(papel.primaria)),
                    ("estrangeira", Json::Bool(papel.estrangeira)),
                    (
                        "composta",
                        Json::Bool(papel.primaria_composta || papel.estrangeira_composta),
                    ),
                    (
                        "nas_chaves_estrangeiras",
                        Json::Lista(papel.chaves_estrangeiras.iter().map(Json::texto_de).collect()),
                    ),
                    (
                        "nos_indices",
                        Json::Lista(papel.indices.iter().map(Json::texto_de).collect()),
                    ),
                ])
            })
            .collect();'''
assert s.count(v) == 1
s = s.replace(v, n)

v = '''                Json::objeto(vec![
                    ("nome", Json::texto_de(&i.nome)),
                    ("unico", Json::Bool(i.unico)),'''
n = '''                Json::objeto(vec![
                    ("nome", Json::texto_de(&i.nome)),
                    ("unico", Json::Bool(i.unico)),
                    ("primario", Json::Bool(i.primario)),
                    ("composto", Json::Bool(i.composta())),'''
assert s.count(v) == 1
s = s.replace(v, n)
p.write_text(s)
print('ok')
