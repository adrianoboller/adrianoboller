# Add Schema::renomear and build
# 28/08 15:38

p='crates/phxsql-core/src/schema.rs'
s=open(p).read()
a='''    pub fn nome(&self) -> &str {'''
b='''    /// Troca o nome da tabela sem mexer em mais nada.
    ///
    /// Existe para a criacao separar `filial.clientes` em schema e tabela: o
    /// esquema chega com o nome qualificado e o que vai para o disco e so a
    /// parte da tabela -- o schema ja e o diretorio.
    pub fn renomear(&mut self, nome: &str) {
        self.nome = nome.to_string();
    }

    pub fn nome(&self) -> &str {'''
assert a in s; s=s.replace(a,b,1)
open(p,'w').write(s)
