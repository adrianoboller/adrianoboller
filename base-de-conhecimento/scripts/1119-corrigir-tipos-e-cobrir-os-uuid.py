# Corrigir tipos e cobrir os UUID
# 29/08 11:38

import io
p='crates/phxsql-server/src/dblink/sincronia.rs'
s=io.open(p,encoding='utf-8').read()
s=s.replace('IndexColumn::asc(pk as u16)','IndexColumn::asc(pk)')
# os dois UUID que faltavam no valor_para_sql
velho='''        Value::Bin(b) => {'''
novo='''        Value::Uuid(b) => {
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
        }
        Value::Bin(b) => {'''
assert s.count(velho)==1
s=s.replace(velho,novo)
io.open(p,'w',encoding='utf-8').write(s)
print('ok')
