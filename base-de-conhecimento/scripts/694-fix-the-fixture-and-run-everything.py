# Fix the fixture and run everything
# 28/08 19:03

import io
p='crates/phxsql-store/tests/alfanumerica.rs'
s=io.open(p,encoding='utf-8').read()
s=s.replace('''    for n in ["Silva", "Adriano", "Zeus", "#etc"] {
        t.inserir(&linha(1, n)).unwrap();
    }''','''    for (i, n) in ["Silva", "Adriano", "Zeus", "#etc"].iter().enumerate() {
        t.inserir(&linha(i as i64 + 1, n)).unwrap();
    }''',1)
io.open(p,'w',encoding='utf-8').write(s)
