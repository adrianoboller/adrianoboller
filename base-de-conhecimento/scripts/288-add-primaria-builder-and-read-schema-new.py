# Add primaria builder and read Schema::new
# 28/08 11:13

import pathlib
p = pathlib.Path('crates/phxsql-core/src/schema.rs')
s = p.read_text()
v = '''    pub fn unico(mut self) -> Self {
        self.unico = true;
        self
    }
}'''
n = '''    pub fn unico(mut self) -> Self {
        self.unico = true;
        self
    }

    /// Marca como chave primaria. Primaria implica unica -- nao ha chave
    /// primaria que aceite duplicata.
    pub fn primaria(mut self) -> Self {
        self.primario = true;
        self.unico = true;
        self
    }

    /// A chave e composta quando tem mais de uma coluna.
    pub fn composta(&self) -> bool {
        self.colunas.len() > 1
    }
}'''
assert s.count(v) == 1
p.write_text(s.replace(v, n))
print('ok')
