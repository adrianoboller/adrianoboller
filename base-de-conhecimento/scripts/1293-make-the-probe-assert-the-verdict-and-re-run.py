# Make the probe assert the verdict and re-run
# 30/08 15:42

p='bancada/arm/sonda.py'
s=open(p).read()
velho='''chave = next((k for k, v in q.items() if isinstance(v, list)), None)
linhas = q.get(chave) or []
print("  lidas de volta:", len(linhas), "(campo %r)" % chave)
if linhas: print("  primeira:", json.dumps(linhas[0], ensure_ascii=False)[:140])
else: print("  resposta crua:", json.dumps(q, ensure_ascii=False)[:220])'''
novo='''# O `varrer` responde dentro de "resultado", com a contagem separada do que
# devolveu -- e e a contagem que fecha a prova: 50 gravadas, 50 devolvidas.
res = q.get("resultado", {})
lidas = res.get("devolvidas", 0)
print("  registros na tabela:", res.get("registros"), "| devolvidas:", lidas)
linhas = next((v for v in res.values() if isinstance(v, list)), [])
if linhas:
    print("  primeira:", json.dumps(linhas[0], ensure_ascii=False)[:140])
falhou = not (q.get("ok") and res.get("registros") == 50 and lidas == 50 and n == 50)
print("VEREDITO:", "REPROVOU" if falhou else "o binario ARM64 gravou e leu 50 linhas")
sys.exit(1 if falhou else 0)'''
assert s.count(velho)==1
s=s.replace(velho,novo)
if 'import sys' not in s:
    s=s.replace('import json, socket, time','import json, socket, sys, time')
open(p,'w').write(s)
print("sonda fecha o veredito na contagem")
