# Usar o to_string canonico dos dois
# 29/08 11:38

import io
p='crates/phxsql-server/src/dblink/sincronia.rs'
s=io.open(p,encoding='utf-8').read()
s=s.replace('''        Value::Uuid(b) => {
            // O outro lado nao tem tipo UUID de 128 bits garantido: viaja como
            // texto canonico, que qualquer VARCHAR(36) recebe.
            literal(&phxsql_core::value::uuid_para_texto(b))?
        }
        Value::Uuid256(b) => {
            let mut s = String::with_capacity(64);
            for byte in b.iter() {
                s.push_str(&format!("{byte:02x}"));
            }
            literal(&s)?
        }''','''        // Os identificadores viajam como o texto canonico deles: o outro lado
        // nao tem tipo UUID garantido, e qualquer VARCHAR os recebe.
        Value::Uuid(u) => literal(&u.to_string())?,
        Value::Uuid256(u) => literal(&u.to_string())?,''')
io.open(p,'w',encoding='utf-8').write(s)
print('ok')
