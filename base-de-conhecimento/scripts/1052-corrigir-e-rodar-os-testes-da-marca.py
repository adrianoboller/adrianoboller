# Corrigir e rodar os testes da marca
# 29/08 03:54

import io
p='crates/phxsql-store/tests/replicacao.rs'
s=io.open(p,encoding='utf-8').read()
velho = '''    let mut esq = esquema();
    esq.paginacao = phxsql_core::paginacao::Paginacao::por_arquivo(200);
    let mut t = Table::criar(dir.0.join("s"), esq).unwrap();'''
novo = '''    let esq = esquema()
        .com_paginacao(phxsql_core::paginacao::Paginacao::nova(200, 0).unwrap())
        .unwrap();
    let mut t = Table::criar(dir.0.join("s"), esq).unwrap();'''
assert s.count(velho)==1
io.open(p,'w',encoding='utf-8').write(s.replace(velho,novo))
