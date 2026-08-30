# Include the raw schema in posicao
# 28/08 20:15

import pathlib
p = pathlib.Path("crates/phxsql-server/src/servidor.rs")
s = p.read_text()
antigo = """        let mut posicoes = Vec::new();
        for nome in db.todas_as_tabelas()? {
            let mut t = db.abrir_qualificada(&nome)?;
            posicoes.push((nome, Json::de_u64(t.eventos()?)));
        }"""
novo = """        let com_esquema = p.booleano_ou("com_esquema", false);
        let mut posicoes = Vec::new();
        for nome in db.todas_as_tabelas()? {
            let mut t = db.abrir_qualificada(&nome)?;
            let mut campos = vec![
                ("eventos".to_string(), Json::de_u64(t.eventos()?)),
                ("registros".to_string(), Json::de_u64(t.registros())),
            ];
            if com_esquema {
                // O bloco de esquema CRU, do jeito que mora no `.reg`. A
                // replica desserializa o mesmo bloco e cria a tabela dela --
                // sem remontar coluna por coluna a partir de JSON, que e onde
                // um tipo ou uma escala se perderiam sem ninguem notar.
                campos.push((
                    "esquema".to_string(),
                    Json::texto_de(bytes_para_hex(&t.esquema().serializar())),
                ));
            }
            posicoes.push((nome, Json::Objeto(campos)));
        }"""
assert antigo in s
s = s.replace(antigo, novo)
p.write_text(s)
print("ok")
