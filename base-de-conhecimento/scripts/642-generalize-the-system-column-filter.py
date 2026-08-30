# Generalize the system-column filter
# 28/08 18:28

import io
p='crates/phxsql-core/src/schema.rs'
s=io.open(p,encoding='utf-8').read()
velho='''pub const COLUNA_ROWNUM: &str = "rownum";'''
novo='''pub const COLUNA_ROWNUM: &str = "rownum";

/// Este nome e de uma coluna do motor?
///
/// Existe para os lugares que precisam ESCONDER as colunas de sistema --
/// a grade, o formulario, a juncao -- nao terem cada um a sua lista. Coluna
/// de sistema nova entra aqui e some dos tres de uma vez; a lista repetida em
/// tres lugares e onde a quarta seria esquecida.
pub fn e_coluna_de_sistema(nome: &str) -> bool {
    nome == COLUNA_SOFTDELETED || nome == COLUNA_ROWNUM
}'''
assert velho in s
s=s.replace(velho,novo,1)
io.open(p,'w',encoding='utf-8').write(s)
