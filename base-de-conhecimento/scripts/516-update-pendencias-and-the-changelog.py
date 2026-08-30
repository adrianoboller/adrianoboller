# Update PENDENCIAS and the changelog
# 28/08 16:43

from collections import Counter
s=open('docs/PENDENCIAS.md').read()
tab=[l for l in s.split('\n') if l.startswith(('| ☑️ |','| ◐ |','| ☐ |'))]
c=Counter(l.split('|')[1].strip() for l in tab)
nums=[int(l.split('|')[2].strip()) for l in tab]
print(c,'total',sum(c.values()),'| repetidos:',[n for n,k in Counter(nums).items() if k>1],'| faltando:',sorted(set(range(1,max(nums)+1))-set(nums)))
