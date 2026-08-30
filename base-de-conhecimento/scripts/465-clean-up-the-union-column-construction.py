# Clean up the union column construction
# 28/08 15:25

p='crates/phxsql-server/src/juncao.rs'
s=open(p).read()
a='''    let colunas = primeiro
        .colunas()
        .iter()
        .enumerate()
        .map(|(i, c)| ColunaSaida {
            nome: c.nome.clone(),
            ty: c.ty.clone(),
            lado: "uniao",
            chave: false,
            // A posição é o que liga as partes; guardar o índice não é preciso
            // porque a ordem da saída é a da primeira parte.
        })
        .map(|c| ColunaSaida { chave: false, ..c })
        .collect::<Vec<_>>();
    let _ = &colunas;
'''
b='''    // Os nomes saem da primeira parte, como no SQL. As outras contribuem
    // linhas, não cabeçalho.
    let colunas: Vec<ColunaSaida> = primeiro
        .colunas()
        .iter()
        .map(|c| ColunaSaida {
            nome: c.nome.clone(),
            ty: c.ty.clone(),
            lado: "uniao",
            chave: false,
        })
        .collect();
'''
assert a in s; s=s.replace(a,b,1)
open(p,'w').write(s)
