# Fix the double count and add the scope guard
# 28/08 16:31

# O guarda AoSair: roda a limpeza saia por onde sair.
p='crates/phxsql-server/src/servidor.rs'
s=open(p).read()
a='''/// Os campos do log que saem do PEDIDO, e nao do resultado.'''
b='''/// Roda a limpeza na saida do escopo, por qualquer caminho.
///
/// Existe por causa dos `return` no meio do laco da conexao: sem ele, cada um
/// deles precisaria lembrar de tirar a conexao do registro, e o dia em que
/// alguem acrescentasse um `return` novo a lista passaria a mostrar conexao
/// que ja morreu -- uma lista que mente e pior do que nenhuma.
struct AoSair<F: FnMut()>(F);

impl<F: FnMut()> Drop for AoSair<F> {
    fn drop(&mut self) {
        (self.0)();
    }
}

/// Os campos do log que saem do PEDIDO, e nao do resultado.'''
assert a in s; s=s.replace(a,b,1)
open(p,'w').write(s)
