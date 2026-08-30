# Herdar sincronias e achar o despachar
# 29/08 11:35

import io
p='crates/phxsql-server/src/dblink/mod.rs'
s=io.open(p,encoding='utf-8').read()
velho='''    pub fn com_a_senha_de(mut self, outra: &Definicao) -> Definicao {
        self.senha = outra.senha.clone();
        self.senha_env = outra.senha_env.clone();
        self
    }'''
novo='''    pub fn com_a_senha_de(mut self, outra: &Definicao) -> Definicao {
        self.senha = outra.senha.clone();
        self.senha_env = outra.senha_env.clone();
        self
    }

    /// Herda as tabelas ligadas de uma definicao anterior.
    ///
    /// Mesmo desenho do `com_a_senha_de`, pela mesma armadilha: a tela salva a
    /// ligacao sem mandar as sincronias, e um salvar comum nao pode apagar o
    /// que o assistente montou.
    pub fn com_as_sincronias_de(mut self, outra: &Definicao) -> Definicao {
        self.sincronias = outra.sincronias.clone();
        self
    }'''
assert s.count(velho)==1
io.open(p,'w',encoding='utf-8').write(s.replace(velho,novo))
print('ok')
