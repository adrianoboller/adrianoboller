# Verify the counts are measured, not guessed
# 28/08 15:09

import re
s=open('docs/PENDENCIAS.md').read()
tab=[l for l in s.split('\n') if l.startswith('| ☑️ |') or l.startswith('| ◐ |') or l.startswith('| ☐ |')]
from collections import Counter
c=Counter(l.split('|')[1].strip() for l in tab)
print(c, 'total', sum(c.values()))
nums=[int(l.split('|')[2].strip()) for l in tab]
print('numeros repetidos:', [n for n,k in Counter(nums).items() if k>1])
print('faltando:', sorted(set(range(1,max(nums)+1))-set(nums)))
