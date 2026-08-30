# Atualizar os resultados da bancada de replicacao
# 29/08 03:55

import io,json
p='bancada/replicacao/resultados.json'
d=json.load(io.open(p,encoding='utf-8'))
d['quando']='2026-08-29'
d['master_linhas_s']=34048
d['replica_eventos_s']=17450
d['alcance_s']=5.7
d['atraso_ms']={"1 insercao":2028,"1.000 insercoes em lote":177,"1 alteracao":2016,
                "1 exclusao suave":165,"1 restauracao":293,"1 exclusao fisica":140,
                "1 linha com memo de 200 KB":144}
for k in ('retomada_subiu_ms','retomada_alcance_s'):
    if k in d: pass
d['retomada_subiu_ms']=323
d['retomada_alcance_s']=0.3
json.dump(d, io.open(p,'w',encoding='utf-8'), ensure_ascii=False, indent=2)
print(json.dumps(d, ensure_ascii=False, indent=2)[:900])
