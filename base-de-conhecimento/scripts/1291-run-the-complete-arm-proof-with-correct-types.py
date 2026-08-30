# Run the complete ARM proof with correct types
# 30/08 15:41

p='/tmp/claude-0/-home-user-adrianoboller/34595649-0af6-575a-8f79-80dbe8cb7a5d/scratchpad/arm-prova.py'
s=open(p).read()
velho='''r = call({"op": "criar_tabela", "database": "iot", "tabela": "leituras",
          "colunas": [{"nome": "sensor", "tipo": "Texto", "tam": 32},
                      {"nome": "valor", "tipo": "Decimal", "precisao": 10, "escala": 2}]})
print("  criar_tabela:", r.get("ok"), "" if r.get("ok") else json.dumps(r)[:180])'''
novo='''d = call({"op": "criar_database", "database": "iot"})
print("  criar_database:", d.get("ok"), "" if d.get("ok") else json.dumps(d)[:160])
r = call({"op": "criar_tabela", "database": "iot", "tabela": "leituras",
          "colunas": [{"nome": "sensor", "tipo": "Str(32)"},
                      {"nome": "valor", "tipo": "Decimal(10,2)"}]})
print("  criar_tabela:", r.get("ok"), "" if r.get("ok") else json.dumps(r)[:180])'''
assert s.count(velho)==1
open(p,'w').write(s.replace(velho,novo))
print("tipos corrigidos para Str(32) e Decimal(10,2)")
