# Mark the system column in op_esquema too
# 28/08 17:58

import io
p='crates/phxsql-server/src/servidor.rs'
s=io.open(p,encoding='utf-8').read()
velho='''                    ("tamanho", Json::de_u64(largura_do_tipo(&c.ty))),
                    ("nullable", Json::Bool(c.nullable)),'''
novo='''                    ("tamanho", Json::de_u64(largura_do_tipo(&c.ty))),
                    ("nullable", Json::Bool(c.nullable)),
                    // Coluna do MOTOR: a tela nao a oferece como campo de
                    // formulario. Quem manda nela e o botao de excluir.
                    ("sistema", Json::Bool(Some(i) == e.coluna_softdeleted())),'''
assert velho in s
s=s.replace(velho,novo,1)
io.open(p,'w',encoding='utf-8').write(s)
