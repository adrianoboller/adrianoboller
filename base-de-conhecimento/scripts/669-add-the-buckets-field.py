# Add the buckets field
# 28/08 18:45

import io
p='crates/phxsql-store/src/reg.rs'
s=io.open(p,encoding='utf-8').read()

# 1. campo novo
velho='''    /// Onde cada volume comeca, quando a particao e por periodo.
    ///
    /// Indice do vetor = volume - 1. Vazio quando a particao e por quantidade,
    /// porque ali o volume sai de uma divisao e nao ha o que guardar.
    fronteiras: Vec<Fronteira>,
}'''
novo='''    /// Onde cada volume comeca, quando a particao e por periodo.
    ///
    /// Indice do vetor = volume - 1. Vazio quando a particao e por quantidade,
    /// porque ali o volume sai de uma divisao e nao ha o que guardar.
    fronteiras: Vec<Fronteira>,
    /// Slots ja usados em cada balde da particao alfanumerica.
    ///
    /// Indice do vetor = balde - 1, com 37 posicoes fixas. Vazio nos outros
    /// modos. Cada balde tem o proprio contador porque a linha vai para o
    /// volume DELA: um contador global nao diria em que slot do `_S` a proxima
    /// Silva entra.
    baldes: Vec<u64>,
}'''
assert velho in s
s=s.replace(velho,novo,1)

# 2. criar e abrir inicializam
s=s.replace('''            proximo_rownum: 1,''','''            proximo_rownum: 1,
            baldes: Vec::new(),''',1)
io.open(p,'w',encoding='utf-8').write(s)
