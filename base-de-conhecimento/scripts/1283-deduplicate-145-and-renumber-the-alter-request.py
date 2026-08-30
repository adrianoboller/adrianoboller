# Deduplicate 145 and renumber the ALTER request
# 30/08 06:45

import re
p='docs/PENDENCIAS.md'
ls=open(p,encoding='utf-8').read().split('\n')

# 145: fica a versao que ja carrega a conta maior de guardas; a outra e a copia
# que o merge trouxe. E os dois numeros dela viraram medidos, nao lembrados.
a,b = ls[157], ls[158]
fica, sai = (157,158) if '27 guardas' in a else (158,157)
del ls[sai]
p145 = ls[fica if fica < sai else fica-1]
p145 = re.sub(r'\*\*(dezesseis|dezessete|dezoito|dezenove|vinte)\*\* partes', '**vinte** partes', p145)
p145 = re.sub(r'Medido: [0-9]+ guardas[^.]*\.', 'Medido: **37 guardas** no catalogo depois de integrar as seis frentes.', p145)
ls[fica if fica < sai else fica-1] = p145

# 147: duas frentes pediram o mesmo numero. A da trava entrou primeiro e fica
# com ele; o ALTER vira 148, que estava livre.
for i,l in enumerate(ls):
    if l.startswith('| ☑️ | 147 |') and 'ALTER TABLE ADD COLUMN' in l:
        ls[i] = l.replace('| 147 |', '| 148 |', 1)
        print('ALTER TABLE renumerado de 147 para 148')
        break
open(p,'w',encoding='utf-8').write('\n'.join(ls))
