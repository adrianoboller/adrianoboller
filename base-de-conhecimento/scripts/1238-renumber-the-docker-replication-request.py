# Renumber the Docker replication request
# 30/08 04:09

import io,re
p="phxsql/docs/PENDENCIAS.md"
linhas=io.open(p,encoding="utf-8").read().split("\n")
i=164  # a linha 165 (1-based) e a da replicacao em Docker
m=re.match(r"^(\| [☑◐]\S* \| )(\d+)( \|)", linhas[i])
assert m and m.group(2)=="142", linhas[i][:70]
linhas[i]=f"{m.group(1)}146{m.group(3)}{linhas[i][m.end():]}"
print("replicacao em Docker: 142 -> 146")
io.open(p,"w",encoding="utf-8").write("\n".join(linhas))
