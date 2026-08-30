# Add multitela.js to the checker's source list
# 30/08 05:21

import re,io
p='crates/phxsql-server/src/conferidor.rs'
s=open(p,encoding='utf-8').read()

velho = '''    (
        "ui/grid/phx-grid.js",
        include_str!("../ui/grid/phx-grid.js"),
    ),
];'''
novo = '''    ("ui/multitela.js", include_str!("../ui/multitela.js")),
    (
        "ui/grid/phx-grid.js",
        include_str!("../ui/grid/phx-grid.js"),
    ),
];'''
assert s.count(velho)==1, s.count(velho)
s=s.replace(velho,novo)

# O comentario prometia o que nao cumpria: a lista era digitada, e nada
# obrigava ela a acompanhar o http.rs.
c_velho = '''/// Os arquivos de interface, embutidos aqui pelo mesmo `include_str!` que o
/// servidor usa para servi-los -- assim nao ha como o conferidor medir uma
/// pagina e o binario servir outra.'''
c_novo = '''/// Os arquivos de interface, embutidos aqui pelo mesmo `include_str!` que o
/// servidor usa para servi-los -- assim nao ha como o conferidor medir uma
/// pagina e o binario servir outra.
///
/// Essa frase era promessa e nao garantia ate a lista ganhar guarda. Ela e
/// digitada, e o `multitela.js` entrou no `http.rs` sem entrar aqui: 1.474
/// linhas de interface servidas ao navegador e invisiveis para a catraca. Quem
/// impede a repeticao e `a_lista_cobre_tudo_que_o_http_serve`, que le o fonte
/// do `http.rs` e reprova o arquivo servido que ninguem mede.'''
assert s.count(c_velho)==1
s=s.replace(c_velho,c_novo)
open(p,'w',encoding='utf-8').write(s)
print("FONTES: multitela.js acrescentado e o comentario deixou de prometer o que nao tinha")
