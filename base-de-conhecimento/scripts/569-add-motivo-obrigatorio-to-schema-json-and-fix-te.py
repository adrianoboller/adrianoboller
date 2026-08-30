# Add motivo_obrigatorio to schema JSON and fix test
# 28/08 17:36

import io
p='crates/phxsql-server/src/valores.rs'
s=io.open(p,encoding='utf-8').read()
velho='''    let esquema = Schema::new(nome, colunas, indices)?;
'''
novo='''    // A tela de criar tabela marca "exigir motivo": a partir dai nenhuma
    // exclusao nesta tabela passa sem uma frase escrita.
    let esquema = Schema::new(nome, colunas, indices)?
        .com_motivo_obrigatorio(j.booleano_ou("motivo_obrigatorio", false));
'''
assert velho in s
s=s.replace(velho,novo,1)

velho2='''        assert_eq!(e.nome(), "pedidos");
        assert_eq!(e.colunas().len(), 3);'''
novo2='''        assert_eq!(e.nome(), "pedidos");
        // Tres declaradas mais a coluna de sistema, que entra sozinha no fim.
        assert_eq!(e.colunas().len(), 4);
        assert_eq!(e.coluna_softdeleted(), Some(3));'''
assert velho2 in s
s=s.replace(velho2,novo2,1)
io.open(p,'w',encoding='utf-8').write(s)
