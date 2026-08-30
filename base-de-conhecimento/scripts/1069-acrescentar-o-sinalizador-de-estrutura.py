# Acrescentar o sinalizador de estrutura
# 29/08 05:17

import io
p='crates/phxsql-store/src/ndx.rs'
s=io.open(p,encoding='utf-8').read()

# 1. o campo
velho='''    cache: CachePaginas,
    gravacoes: u64,
}'''
novo='''    cache: CachePaginas,
    gravacoes: u64,
    /// A pagina 0 esta atrasada em relacao ao que ha em RAM.
    ///
    /// O cabecalho guarda quatro coisas: `qtd_paginas`, `pagina_livre`, a raiz
    /// de cada indice e a `qtd_chaves` de cada indice. As tres primeiras sao
    /// ESTRUTURA -- sem elas a arvore nao se acha -- e mudam raramente: uma
    /// alocacao de pagina a cada ~118 chaves, uma troca de raiz a cada nivel
    /// novo. A quarta e um CONTADOR, e `verificar` sabe recalcula-lo varrendo.
    ///
    /// Antes, toda chave inserida gravava 4 KiB no offset 0 -- com dois
    /// indices, 8 KiB por linha, so para adiantar um contador. E a terceira vez
    /// que este projeto encontra o mesmo defeito: o `.reg` reserializava o
    /// esquema por linha (DESEMPENHO.md 2.0) e o `.log` gravava o cabecalho por
    /// evento (2.2). **Cabecalho de arquivo nao pertence ao caminho quente.**
    estrutura_mudou: bool,
}'''
assert s.count(velho)==1
s=s.replace(velho,novo)

# 2. os construtores
s=s.replace('''            cache: CachePaginas::nova(cache_paginas()),
            gravacoes: 0,
        };''','''            cache: CachePaginas::nova(cache_paginas()),
            gravacoes: 0,
            estrutura_mudou: false,
        };''')
io.open(p,'w',encoding='utf-8').write(s)
print('campos ok')
