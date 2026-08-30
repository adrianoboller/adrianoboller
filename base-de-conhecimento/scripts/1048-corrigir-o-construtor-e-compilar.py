# Corrigir o construtor e compilar
# 29/08 03:46

import io
p='crates/phxsql-store/src/log.rs'
s=io.open(p,encoding='utf-8').read()
velho='''            volumes,
            cabs: HashMap::new(),
            volume_atual,
            usuario: 0,
        };'''
novo='''            volumes,
            cabs: HashMap::new(),
            volume_atual,
            marca: None,
            usuario: 0,
        };'''
assert s.count(velho)==1
io.open(p,'w',encoding='utf-8').write(s.replace(velho,novo))
