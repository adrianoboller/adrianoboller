# Find the right response field and complete the read-back
# 30/08 15:41

p='/tmp/claude-0/-home-user-adrianoboller/34595649-0af6-575a-8f79-80dbe8cb7a5d/scratchpad/arm-prova.py'
s=open(p).read()
velho='''linhas = q.get("linhas") or q.get("dados") or []
print("  lidas de volta:", len(linhas))
if linhas: print("  primeira:", json.dumps(linhas[0], ensure_ascii=False)[:140])'''
novo='''chave = next((k for k, v in q.items() if isinstance(v, list)), None)
linhas = q.get(chave) or []
print("  lidas de volta:", len(linhas), "(campo %r)" % chave)
if linhas: print("  primeira:", json.dumps(linhas[0], ensure_ascii=False)[:140])
else: print("  resposta crua:", json.dumps(q, ensure_ascii=False)[:220])'''
assert s.count(velho)==1
open(p,'w').write(s.replace(velho,novo))
