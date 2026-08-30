# Clean warnings and run clippy
# 30/08 06:36

p='crates/phxsql-server/src/replica.rs'
s=open(p,encoding='utf-8').read()
s=s.replace('use std::io::{BufRead, BufReader, Read, Write};','use std::io::{BufRead, BufReader, Write};')
import re
m=re.search(r'(?:^///[^\n]*\n)*^const TETO_DA_RESPOSTA: u64 = [^\n]*\n', s, re.M)
assert m, "constante nao achada"
s = s[:m.start()] + (
    '// O teto de um registro lido do fio NAO mora mais aqui: ele desceu para o\n'
    '// `Canal` do `phxsql-core` (`TETO_DO_REGISTRO`), porque a leitura passou a\n'
    '// ser dele. Um teto nesta camada voltaria a deixar o caminho cifrado sem\n'
    '// nenhum -- que foi exatamente o risco desta integracao.\n'
) + s[m.end():]
open(p,'w',encoding='utf-8').write(s)
print("import e constante limpos, com o rastro de para onde o teto foi")
