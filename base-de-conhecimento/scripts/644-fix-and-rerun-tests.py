# Fix and rerun tests
# 28/08 18:28

import io
p='crates/phxsql-server/src/juncao.rs'
s=io.open(p,encoding='utf-8').read()
velho='''    // Mesma razao da juncao: a coluna de sistema seria falso em toda linha.
    if r.colunas
        .last()
        .is_some_and(|c| c.nome == COLUNA_SOFTDELETED)
    {
        r.colunas.pop();
        for linha in &mut r.linhas {
            linha.pop();
        }
    }
    Ok(r)'''
novo='''    // Mesma razao da juncao. As de sistema estao no FIM, entao sair de tras
    // para a frente basta.
    while r.colunas.last().is_some_and(|c| e_coluna_de_sistema(&c.nome)) {
        r.colunas.pop();
        for linha in &mut r.linhas {
            linha.pop();
        }
    }
    Ok(r)'''
assert velho in s
s=s.replace(velho,novo,1)
io.open(p,'w',encoding='utf-8').write(s)
