# Simular queda de verdade e nao limpar marca na espiada
# 29/08 06:04

import io
p='crates/phxsql-store/tests/ndx.rs'
s=io.open(p,encoding='utf-8').read()
velho='''        // De proposito SEM `sincronizar`: o que se quer provar e que o `write`
        // ja aconteceu, e nao que o `fsync` salvou o dia.
    }'''
novo='''        // `forget` em vez de `drop`: e o que simula a QUEDA. Um `drop` normal
        // roda o fechamento limpo e leva as paginas ao arquivo -- que e o
        // comportamento certo, e tem teste proprio. O que se quer aqui e o
        // processo morrendo sem fechar nada.
        std::mem::forget(n);
    }'''
assert s.count(velho)==1
s=s.replace(velho,novo)
# a espiada tambem: ela nao pode ser um drop limpo
velho2='''        let espiada = NdxFile::abrir(&caminho).unwrap();
        assert!(
            espiada.precisa_reconstruir(),
            "a marca tem de estar no disco ANTES do sincronizar, e nao depois"
        );
        drop(espiada);
        n.sincronizar().unwrap();'''
novo2='''        let espiada = NdxFile::abrir(&caminho).unwrap();
        assert!(
            espiada.precisa_reconstruir(),
            "a marca tem de estar no disco ANTES do sincronizar, e nao depois"
        );
        // E fechar a espiada NAO pode limpar a marca: ninguem reconstruiu nada.
        drop(espiada);
        assert!(NdxFile::abrir(&caminho).unwrap().precisa_reconstruir());
        n.sincronizar().unwrap();'''
assert s.count(velho2)==1
s=s.replace(velho2,novo2)
io.open(p,'w',encoding='utf-8').write(s)
print('teste ok')
