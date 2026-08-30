# Authenticate and write real rows on the ARM server
# 30/08 15:38

p='/tmp/claude-0/-home-user-adrianoboller/34595649-0af6-575a-8f79-80dbe8cb7a5d/scratchpad/prova-arm.sh'
s=open(p).read()
velho='''    r=call({"op":"ping"}); print("  resposta crua:", json.dumps(r)[:300])'''
novo='''import os
tok=json.load(open("config.json")).get("token","")
def c(d):
    d["token"]=tok; return call(d)
print("  ping:", c({"op":"ping"}).get("ok"))
print("  criar:", c({"op":"criar_tabela","database":"iot","tabela":"leituras",
    "colunas":[{"nome":"sensor","tipo":"Texto","tam":32},
               {"nome":"valor","tipo":"Decimal","precisao":10,"escala":2}]}).get("ok"))
for i in range(50):
    r=c({"op":"inserir","database":"iot","tabela":"leituras",
         "valores":{"sensor":f"s{i%5}","valor":f"{20+i*0.5:.2f}"}})
    if not r.get("ok"): print("  inserir falhou:", json.dumps(r)[:200]); break
q=c({"op":"varrer","database":"iot","tabela":"leituras","limite":1000})
linhas=q.get("linhas") or q.get("dados") or []
print("  linhas gravadas e lidas em ARM64:", len(linhas))
print("  primeira:", json.dumps(linhas[0])[:120] if linhas else "-")'''
assert s.count(velho)==1
open(p,'w').write(s.replace(velho,novo))
