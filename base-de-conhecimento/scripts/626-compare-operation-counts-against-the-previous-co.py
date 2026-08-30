# Compare operation counts against the previous commit
# 28/08 18:13

import re, subprocess
def ops_de(texto):
    i=texto.index('fn executar(')
    corpo=texto[i:i+30000]
    if 'operacao desconhecida' in corpo: corpo=corpo[:corpo.index('operacao desconhecida')]
    grupos=[]
    for m in re.finditer(r'^\s*((?:"[a-zA-Z_0-9]+"\s*\|\s*)*"[a-zA-Z_0-9]+")\s*=>', corpo, re.M):
        grupos.append(tuple(re.findall(r'"([a-zA-Z_0-9]+)"', m.group(1))))
    return grupos

antes = subprocess.run(['git','show','f0962fc:phxsql/crates/phxsql-server/src/servidor.rs'],
                       capture_output=True, text=True, cwd='/home/user/adrianoboller').stdout
agora = open('/home/user/adrianoboller/phxsql/crates/phxsql-server/src/servidor.rs').read()
a, b = ops_de(antes), ops_de(agora)
print('grupos (operacoes distintas) antes:', len(a), ' agora:', len(b))
novos = [g for g in b if g not in a]
print('novos:', novos)
