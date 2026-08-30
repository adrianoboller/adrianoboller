# Fix Brazilian number parsing and test it
# 28/08 19:25

import io
p='crates/phxsql-server/src/servidor.rs'
s=io.open(p,encoding='utf-8').read()
velho='''                // JSON ja vem tipado; os outros quatro sao texto puro, e a
                // conversao passa a ser dirigida pelo esquema.
                let texto_puro = f != crate::importar::Formato::Json;
                (carga.para_json(), f.nome().to_string(), texto_puro)'''
novo='''                // Carga COLADA e sempre convertida pelo esquema, nos cinco
                // formatos. Uma regra so, e nao duas: o leitor entrega texto,
                // e quem sabe que aquilo e um inteiro e a coluna. Vale para o
                // JSON tambem -- `{"id":1}` chega aqui como `"1"` e volta a
                // ser inteiro pelo tipo declarado.
                (carga.para_json(), f.nome().to_string(), true)'''
assert velho in s
s=s.replace(velho,novo,1)
io.open(p,'w',encoding='utf-8').write(s)
