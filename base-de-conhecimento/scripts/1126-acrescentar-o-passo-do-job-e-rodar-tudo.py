# Acrescentar o passo do job e rodar tudo
# 29/08 11:44

import io
p='bancada/dblink/prova-sincronia.py'
s=io.open(p,encoding='utf-8').read()
velho='''print("\\nPROVA COMPLETA: os dois lados convergem, o dono vence, e o limite da")
print("exclusao e real -- documentado e conferido, nao so escrito.")'''
novo='''print("== 7. o job roda a sincronia sozinho ==")
fala({"op": "job_salvar", "nome": "sincronia-crm",
      "descricao": "convergencia com o MySQL(R) da bancada",
      "cada_minutos": 5, "ligado": True, "usuario": "adriano",
      "pedido": {"op": "dblink_sincronizar", "dblink": "crm"}})
mysql("INSERT INTO clientes VALUES (7,'Chegou Pelo Job','Itajai',1.00,'2026-08-29')")
r = fala({"op": "job_rodar", "nome": "sincronia-crm"})
corrida = r.get("resultado", r)
print(f"  ok  job rodou: {json.dumps(corrida, ensure_ascii=False)[:100]}")
achado = fala({"op": "buscar", "database": "espelho", "tabela": "clientes",
               "indice": "porChave", "chave": [7]})
confere("a linha do job chegou aqui", achado["linhas"][0]["nome"], "Chegou Pelo Job")

print("\\nPROVA COMPLETA: os dois lados convergem, o dono vence, o limite da")
print("exclusao e real, e o job faz a rodada sozinho -- cada afirmacao acima")
print("foi conferida contra o resultado, nao so impressa.")'''
assert s.count(velho)==1
io.open(p,'w',encoding='utf-8').write(s.replace(velho,novo))
print('ok')
