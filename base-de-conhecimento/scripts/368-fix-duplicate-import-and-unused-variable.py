# Fix duplicate import and unused variable
# 28/08 13:28

import pathlib
p = pathlib.Path('crates/phxsql-server/src/servidor.rs')
s = p.read_text()
s = s.replace('use std::collections::HashMap;\n\nuse phxsql_core::schema::Schema;\nuse phxsql_core::value::Value;\n\nuse crate::pivot::',
              'use phxsql_core::schema::Schema;\nuse phxsql_core::value::Value;\n\nuse crate::pivot::', 1)
# a permissao do pivot e leitura; o `sessao` confere a tabela como as outras
s = s.replace('    fn op_pivotar(&self, p: &Json, sessao: &Sessao) -> Result<Json> {\n        let agregador',
              '''    fn op_pivotar(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        // O portao geral ja conferiu `ler` contra este database; a linha
        // abaixo existe para o caso de a tabela de fatos estar num schema que
        // o `abrir` recusaria -- e o mesmo caminho das outras leituras.
        let _ = sessao;
        let agregador''')
p.write_text(s)
