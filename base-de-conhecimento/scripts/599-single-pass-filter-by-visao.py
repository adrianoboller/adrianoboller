# Single-pass filter by visao
# 28/08 17:49

import io
p='crates/phxsql-server/src/servidor.rs'
s=io.open(p,encoding='utf-8').read()
velho='''            let todos = t.varrer_indice(&indice)?;
            match visao {
                Visao::Todas => todos,
                Visao::Ativas => t.filtrar_ativos(&todos)?,
                Visao::Excluidas => {
                    let ativos = t.filtrar_ativos(&todos)?;
                    todos.into_iter().filter(|r| !ativos.contains(r)).collect()
                }
            }'''
novo='''            let todos = t.varrer_indice(&indice)?;
            t.filtrar(&todos, visao)?'''
assert velho in s
s=s.replace(velho,novo,1)
io.open(p,'w',encoding='utf-8').write(s)
