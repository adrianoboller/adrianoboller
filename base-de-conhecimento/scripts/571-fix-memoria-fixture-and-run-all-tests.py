# Fix memoria fixture and run all tests
# 28/08 17:36

import io
p='crates/phxsql-store/src/memoria.rs'
s=io.open(p,encoding='utf-8').read()
# as linhas do teste passam a levar a coluna de sistema
velho='''        let nova = vec![
            Value::Int(6),
            Value::Str("Cafe".into()),
            Value::Str("Curitiba".into()),
            Value::Decimal(2100),
        ];'''
novo='''        let nova = vec![
            Value::Int(6),
            Value::Str("Cafe".into()),
            Value::Str("Curitiba".into()),
            Value::Decimal(2100),
            Value::Bool(false),
        ];'''
assert velho in s
s=s.replace(velho,novo,1)
velho2='''        let trocada = vec![
            Value::Int(6),
            Value::Str("Cha".into()),
            Value::Str("Curitiba".into()),
            Value::Decimal(2100),
        ];'''
novo2='''        let trocada = vec![
            Value::Int(6),
            Value::Str("Cha".into()),
            Value::Str("Curitiba".into()),
            Value::Decimal(2100),
            Value::Bool(false),
        ];'''
assert velho2 in s
s=s.replace(velho2,novo2,1)
io.open(p,'w',encoding='utf-8').write(s)
