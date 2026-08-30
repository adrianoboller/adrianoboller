# Fix field name and check Sessao struct
# 28/08 17:42

import io,re
p='crates/phxsql-server/src/servidor.rs'
s=io.open(p,encoding='utf-8').read()
# no bloco de testes_exclusao, trocar "encontrados" por "total" nas varreduras
i=s.index('mod testes_exclusao')
cab, corpo = s[:i], s[i:]
corpo = corpo.replace('v.inteiro_ou("encontrados", -1)','v.inteiro_ou("total", -1)')
# a sessao do portao precisa estar autenticada
corpo = corpo.replace('''        let mut sessao = Sessao {
            usuario: Some(usuario),
            ..Sessao::default()
        };''','''        let mut sessao = Sessao {
            usuario: Some(usuario),
            autenticada: true,
            ..Sessao::default()
        };''')
io.open(p,'w',encoding='utf-8').write(cab+corpo)
