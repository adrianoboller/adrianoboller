# Find the systematic untranslated case
# 30/08 15:50

import re
s=open('crates/phxsql-server/src/idiomas.rs',encoding='utf-8').read()
pat=re.compile(r'texto!\(\s*("(?:[^"\\]|\\.)*")\s*,\s*((?:"(?:[^"\\]|\\.)*"\s*,?\s*){6})\)', re.S)
todos=[]; sistematico=[]
for m in pat.finditer(s):
    nome=m.group(1).strip('"')
    vals=re.findall(r'"((?:[^"\\]|\\.)*)"', m.group(2))
    if len(vals)!=6: continue
    todos.append((nome,vals))
    if len(set(vals))==1 and len(vals[0])>3:
        sistematico.append((nome,vals[0]))
print("chaves:",len(todos))
print("chaves com os SEIS idiomas identicos (o caso que denuncia de verdade):",len(sistematico))
for n,v in sistematico[:12]: print("   ",n,"=",repr(v[:50]))
print()
# O outro lado: frase longa igual e muito mais suspeita que palavra curta.
longas=[(n,v) for n,v in todos if len(v[0])>25 and sum(1 for x in v[1:] if x==v[0])>=3]
print("frases LONGAS (>25 chars) iguais em 3+ idiomas -- suspeita forte:",len(longas))
for n,v in longas[:6]: print("   ",n,"=",repr(v[0][:60]))
