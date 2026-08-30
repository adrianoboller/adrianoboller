# Ligar o modulo e o campo
# 29/08 11:34

import io
p='crates/phxsql-server/src/dblink/mod.rs'
s=io.open(p,encoding='utf-8').read()
s=s.replace('pub mod operacoes;','pub mod operacoes;\npub mod sincronia;')

# o campo na Definicao
s=s.replace('''    pub somente_leitura: bool,
    pub timeout_s: u64,
    pub max_linhas: u64,
}''','''    pub somente_leitura: bool,
    pub timeout_s: u64,
    pub max_linhas: u64,
    /// Tabelas ligadas por sincronia. Campo ausente no arquivo = nenhuma,
    /// entao todo `dblink.json` escrito antes continua abrindo igual.
    pub sincronias: Vec<sincronia::Sincronia>,
}''')

# default
s=s.replace('''            timeout_s: padrao_timeout(),
            max_linhas: padrao_max_linhas(),
        }
    }
}''','''            timeout_s: padrao_timeout(),
            max_linhas: padrao_max_linhas(),
            sincronias: Vec::new(),
        }
    }
}''')
io.open(p,'w',encoding='utf-8').write(s)
print('mod ok')
