# Add primario to IndexDef
# 28/08 11:13

import pathlib
p = pathlib.Path('crates/phxsql-core/src/schema.rs')
s = p.read_text()

# ---------------------------------------------------- IndexDef.primario
v = '''pub struct IndexDef {
    pub nome: String,
    pub colunas: Vec<IndexColumn>,
    pub unico: bool,
}'''
n = '''pub struct IndexDef {
    pub nome: String,
    pub colunas: Vec<IndexColumn>,
    pub unico: bool,
    /// Este e o indice da CHAVE PRIMARIA da tabela.
    ///
    /// Ate aqui o motor so tinha "indice unico", e chave primaria e mais do
    /// que isso: e a identidade da linha, a que as chaves estrangeiras das
    /// outras tabelas apontam, e a que a tela precisa saber para dizer quais
    /// campos formam a chave. So um indice pode ser primario, e ele e sempre
    /// unico -- `Schema::new` recusa o contrario.
    pub primario: bool,
}'''
assert s.count(v) == 1
s = s.replace(v, n)

v = '''        IndexDef {
            nome: nome.into(),
            colunas,
            unico: false,
        }'''
n = '''        IndexDef {
            nome: nome.into(),
            colunas,
            unico: false,
            primario: false,
        }'''
assert s.count(v) == 1
s = s.replace(v, n)
p.write_text(s)
print('ok')
