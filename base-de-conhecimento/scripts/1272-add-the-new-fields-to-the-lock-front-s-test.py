# Add the new fields to the lock front's test
# 30/08 06:37

p='crates/phxsql-server/tests/trava-atras-da-rede.rs'
s=open(p,encoding='utf-8').read()
import re
# Este teste prova o comportamento com o fio EM CLARO -- que e exatamente o
# caso que a cifra tem de deixar intacto. Os padroes certos sao os de quem
# nao pediu cifra nenhuma.
m=re.search(r'(c\.replicacao\.origens = vec!\[Origem \{\n)', s)
assert m
s = s[:m.end()] + (
    '        // A cifra do fio nao entra aqui de proposito: esta guarda mede a\n'
    '        // trava com o fio em claro, que e o caminho que a cifra promete\n'
    '        // deixar como estava.\n'
    '        cifra: false,\n'
    '        chave_do_fio: String::new(),\n'
) + s[m.end():]
open(p,'w',encoding='utf-8').write(s)
print("os dois campos novos entram no teste da trava, em claro e com o motivo")
