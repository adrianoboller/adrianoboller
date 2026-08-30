# Corrigir o default e ver de_json e para_disco
# 29/08 11:34

import io
p='crates/phxsql-server/src/dblink/mod.rs'
s=io.open(p,encoding='utf-8').read()
velho='''            somente_leitura: true,
            timeout_s: 10,
            max_linhas: 1_000,
        }
    }
}'''
novo='''            somente_leitura: true,
            timeout_s: 10,
            max_linhas: 1_000,
            sincronias: Vec::new(),
        }
    }
}'''
assert s.count(velho)==1
s=s.replace(velho,novo)
io.open(p,'w',encoding='utf-8').write(s)
print('default ok')
