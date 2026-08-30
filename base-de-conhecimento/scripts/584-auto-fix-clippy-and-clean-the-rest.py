# Auto-fix clippy and clean the rest
# 28/08 17:43

import io
# 1. o Rng nao usado no teste novo: `mod comum` traz tudo
p='crates/phxsql-store/tests/exclusao.rs'
s=io.open(p,encoding='utf-8').read()
s=s.replace('''mod comum;

use comum::DirTemp;''','''#[allow(dead_code, reason = "o modulo comum serve a varios testes; este usa so o DirTemp")]
mod comum;

use comum::DirTemp;''',1)
io.open(p,'w',encoding='utf-8').write(s)

# 2. as permissoes montadas fora do inicializador
p='crates/phxsql-server/src/servidor.rs'
s=io.open(p,encoding='utf-8').read()
velho='''        let mut permissoes = Permissoes::default();
        permissoes.ler = true;
        permissoes.inserir = true;
        permissoes.alterar = true;
        permissoes.excluir = true;
        permissoes.administrar = false;'''
novo='''        let permissoes = Permissoes {
            ler: true,
            inserir: true,
            alterar: true,
            excluir: true,
            administrar: false,
            ..Permissoes::default()
        };'''
if velho in s:
    s=s.replace(velho,novo,1)
io.open(p,'w',encoding='utf-8').write(s)
