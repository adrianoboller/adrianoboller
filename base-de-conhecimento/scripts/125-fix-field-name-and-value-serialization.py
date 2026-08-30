# Fix field name and value serialization
# 27/08 20:23

p='crates/phxsql-server/src/servidor.rs'
s=open(p).read()
s=s.replace('&esquema.colunas()[coluna].tipo', '&esquema.colunas()[coluna].ty')
s=s.replace('''        let nomes: Vec<String> = if consulta.colunas.is_empty() {
            esquema.colunas().iter().map(|c| c.nome.clone()).collect()
        } else {
            consulta
                .colunas
                .iter()
                .map(|i| esquema.colunas()[*i].nome.clone())
                .collect()
        };''','''        let indices: Vec<usize> = if consulta.colunas.is_empty() {
            (0..esquema.colunas().len()).collect()
        } else {
            consulta.colunas.clone()
        };
        let nomes: Vec<String> = indices
            .iter()
            .map(|i| esquema.colunas()[*i].nome.clone())
            .collect();
        let tipos: Vec<phxsql_core::types::ColumnType> = indices
            .iter()
            .map(|i| esquema.colunas()[*i].ty.clone())
            .collect();''')
s=s.replace('''                            let mut campos = vec![("rowid", Json::de_u64(*rowid))];
                            for (n, v) in nomes.iter().zip(l.iter()) {
                                campos.push((n.as_str(), crate::valores::valor_para_json(v)));
                            }''','''                            let mut campos = vec![("rowid", Json::de_u64(*rowid))];
                            for ((n, v), ty) in nomes.iter().zip(l.iter()).zip(tipos.iter()) {
                                campos.push((n.as_str(), crate::valores::valor_para_json(v, ty)));
                            }''')
open(p,'w').write(s)
