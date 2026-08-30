# Refresh the replication numbers everywhere
# 29/08 02:00

import pathlib, json, re

# 1) o resultados.json da bancada de replicacao
p = pathlib.Path("bancada/replicacao/resultados.json")
d = json.loads(p.read_text())
d.update({
  "master_linhas_s": 28914, "replica_eventos_s": 4357, "alcance_s": 23.0,
  "retomada_subiu_ms": 325, "retomada_alcance_s": 0.7,
  "iguais_no_fim": True, "linhas": 105001, "versao": "0.17.0",
})
p.write_text(json.dumps(d, indent=2, ensure_ascii=False) + "\n")

# 2) os documentos que citam os dois numeros
trocas = [
  ("4.273 eventos/s contra 18.773 linhas/s", "4.357 eventos/s contra 28.914 linhas/s"),
  ("4.273 eventos/s por réplica", "4.357 eventos/s por réplica"),
  ("master 18.773 linhas/s, réplica 4.273 eventos/s", "master 28.914 linhas/s, réplica 4.357 eventos/s"),
  ("| Master, com a imagem no diário | 18.773 linhas/s |", "| Master, com a imagem no diário | 28.914 linhas/s |"),
  ("| Aplicação, por réplica (as três em paralelo) | 4.273 eventos/s |", "| Aplicação, por réplica (as três em paralelo) | 4.357 eventos/s |"),
  ("- **A réplica aplica mais devagar do que o master escreve** — 4.273 eventos/s",
   "- **A réplica aplica mais devagar do que o master escreve** — 4.357 eventos/s"),
]
for f in ["docs/CLUSTER.md","docs/DESEMPENHO.md","docs/HFSQL.md","docs/PENDENCIAS.md","docs/REPLICACAO.md"]:
    q = pathlib.Path(f); s = q.read_text(); antes = s
    for a,b in trocas: s = s.replace(a,b)
    if s != antes: q.write_text(s); print("atualizado:", f)
