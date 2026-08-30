# Fix the memoria fixture and rerun
# 28/08 18:30

import io
p='crates/phxsql-store/src/memoria.rs'
s=io.open(p,encoding='utf-8').read()
s=s.replace('''            Value::Decimal(2100),
            Value::Bool(false),
        ];
        let rowid = t.inserir(&nova).unwrap();
        m.anotar_insercao(rowid, &nova);''','''            Value::Decimal(2100),
            Value::Bool(false),
            Value::UInt(6),
        ];
        let rowid = t.inserir(&nova).unwrap();
        m.anotar_insercao(rowid, &nova);''',1)
s=s.replace('''            Value::Decimal(2100),
            Value::Bool(false),
        ];
        t.atualizar(rowid, &trocada).unwrap();''','''            Value::Decimal(2100),
            Value::Bool(false),
            Value::UInt(6),
        ];
        t.atualizar(rowid, &trocada).unwrap();''',1)
io.open(p,'w',encoding='utf-8').write(s)
