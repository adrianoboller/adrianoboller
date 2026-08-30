# Ver o resultados.json da replicacao
# 29/08 03:55

import io,json
# resultados.json da bancada de replicacao
p='bancada/replicacao/resultados.json'
d=json.load(io.open(p,encoding='utf-8'))
print(json.dumps(d, ensure_ascii=False)[:400])
