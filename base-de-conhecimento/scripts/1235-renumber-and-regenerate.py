# Renumber and regenerate
# 30/08 04:03

import io,re
p="phxsql/docs/PENDENCIAS.md"
linhas=io.open(p,encoding="utf-8").read().split("\n")
nums=[int(m.group(1)) for l in linhas if (m:=re.match(r"^\| [☑◐]\S* \| (\d+) \|", l))]
prox=max(nums)+1
# a bateria (linha 158) fica com o numero novo; o 142 continua sendo o da GPU,
# que ja foi citado no commit dela
i=157
m=re.match(r"^(\| [☑◐]\S* \| )(\d+)( \|)", linhas[i])
assert m and m.group(2)=="142", linhas[i][:60]
linhas[i]=f"{m.group(1)}{prox}{m.group(3)}{linhas[i][m.end():]}"
print("bateria: 142 ->", prox)
io.open(p,"w",encoding="utf-8").write("\n".join(linhas))
