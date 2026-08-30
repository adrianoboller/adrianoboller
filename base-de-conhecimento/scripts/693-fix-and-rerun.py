# Fix and rerun
# 28/08 19:03

import io
p='crates/phxsql-store/tests/alfanumerica.rs'
s=io.open(p,encoding='utf-8').read()
s=s.replace("let inst = Instancia::nova(&base);","let inst = Instancia::nova(&base).unwrap();")
io.open(p,'w',encoding='utf-8').write(s)
