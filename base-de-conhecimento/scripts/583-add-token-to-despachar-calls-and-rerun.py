# Add token to despachar calls and rerun
# 28/08 17:42

import io
p='crates/phxsql-server/src/servidor.rs'
s=io.open(p,encoding='utf-8').read()
i=s.index('mod testes_exclusao')
cab, corpo = s[:i], s[i:]
corpo = corpo.replace('''            autenticada: true,
''','')
corpo = corpo.replace('r#"{"op":"lixeira","database":"b","tabela":"c"}"#',
                      'r#"{"op":"lixeira","token":"t","database":"b","tabela":"c"}"#')
corpo = corpo.replace('r#"{"op":"motivos","database":"b","tabela":"c"}"#',
                      'r#"{"op":"motivos","token":"t","database":"b","tabela":"c"}"#')
corpo = corpo.replace('r#"{"op":"excluir","database":"b","tabela":"c","rowid":1}"#',
                      'r#"{"op":"excluir","token":"t","database":"b","tabela":"c","rowid":1}"#')
io.open(p,'w',encoding='utf-8').write(cab+corpo)
