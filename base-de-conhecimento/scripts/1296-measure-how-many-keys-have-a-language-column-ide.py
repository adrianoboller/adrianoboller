# Measure how many keys have a language column identical to Portuguese
# 30/08 15:50

import re
s=open('crates/phxsql-server/src/idiomas.rs',encoding='utf-8').read()
# texto!(nome, pt, fr, en, it, de, es)
pat=re.compile(r'texto!\(\s*("(?:[^"\\]|\\.)*")\s*,\s*((?:"(?:[^"\\]|\\.)*"\s*,?\s*){6})\)', re.S)
idiomas=['pt','fr','en','it','de','es']
achados=0; iguais={i:0 for i in idiomas[1:]}; exemplos={i:[] for i in idiomas[1:]}
for m in pat.finditer(s):
    nome=m.group(1).strip('"')
    vals=re.findall(r'"((?:[^"\\]|\\.)*)"', m.group(2))
    if len(vals)!=6: continue
    achados+=1
    pt=vals[0]
    for i,lang in enumerate(idiomas[1:],start=1):
        if vals[i]==pt and len(pt)>2:
            iguais[lang]+=1
            if len(exemplos[lang])<3: exemplos[lang].append((nome,pt))
print(f"chaves lidas: {achados}")
print("colunas identicas ao portugues (fora as curtas de 1-2 letras):")
for l in idiomas[1:]:
    print(f"  {l}: {iguais[l]}")
print()
for l in idiomas[1:]:
    if exemplos[l]:
        print(f"  exemplos {l}:", "; ".join(f"{n}={v[:40]!r}" for n,v in exemplos[l]))
