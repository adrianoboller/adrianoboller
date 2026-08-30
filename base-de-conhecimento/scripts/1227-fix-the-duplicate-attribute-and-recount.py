# Fix the duplicate attribute and recount
# 29/08 23:43

import io
# a chave `tela.alternar_tema` deixou de ser usada -- tira da fabrica, senao o
# conferidor de chave morta reprova
p="phxsql/crates/phxsql-server/src/idiomas.rs"
s=io.open(p,encoding="utf-8").read()
linhas=[l for l in s.split("\n") if not l.startswith('    texto!("tela.alternar_tema"')]
io.open(p,"w",encoding="utf-8").write("\n".join(linhas))
print("chave morta removida")
