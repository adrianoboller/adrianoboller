# Count protocol operations
# 28/08 18:13

import io, re
p='crates/phxsql-server/src/servidor.rs'
s=io.open(p,encoding='utf-8').read()
i=s.index('fn executar(')
# o corpo do despacho ate o `_ =>`
corpo=s[i:i+30000]
fim=corpo.index('operacao desconhecida') if 'operacao desconhecida' in corpo else len(corpo)
corpo=corpo[:fim]
ops=set()
for m in re.finditer(r'^\s*((?:"[a-zA-Z_0-9]+"\s*\|\s*)*"[a-zA-Z_0-9]+")\s*=>', corpo, re.M):
    for nome in re.findall(r'"([a-zA-Z_0-9]+)"', m.group(1)):
        ops.add(nome)
print(len(ops), 'operacoes (contando apelidos como uma so? nao -- todas as palavras)')
print(sorted(ops))
