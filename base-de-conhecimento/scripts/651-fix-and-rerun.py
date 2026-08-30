# Fix and rerun
# 28/08 18:32

import io
p='crates/phxsql-store/tests/paginacao.rs'
s=io.open(p,encoding='utf-8').read()
s=s.replace('''    match t.ler(rowid).unwrap().unwrap()[i] {
        Value::UInt(n) => n,
        outro => panic!("rownum nao e UInt: {outro:?}"),
    }''','''    let linha = t.ler(rowid).unwrap().unwrap();
    match &linha[i] {
        Value::UInt(n) => *n,
        outro => panic!("rownum nao e UInt: {outro:?}"),
    }''',1)
io.open(p,'w',encoding='utf-8').write(s)
