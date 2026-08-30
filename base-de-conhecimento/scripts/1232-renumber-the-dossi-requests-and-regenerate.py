# Renumber the dossiê requests and regenerate
# 30/08 03:13

import io,re
p="phxsql/docs/PENDENCIAS.md"
linhas=io.open(p,encoding="utf-8").read().split("\n")
nums=[]
for l in linhas:
    m=re.match(r"^\| [☑◐]\S* \| (\d+) \|", l)
    if m: nums.append(int(m.group(1)))
prox=max(nums)+1
print("proximo livre:", prox)
for i,l in enumerate(linhas):
    if i+1 in (161,162):
        m=re.match(r"^(\| [☑◐]\S* \| )(\d+)( \|)", l)
        assert m, l[:60]
        linhas[i]=f"{m.group(1)}{prox}{m.group(3)}{l[m.end():]}"
        print("linha",i+1,"->",prox)
        prox+=1
io.open(p,"w",encoding="utf-8").write("\n".join(linhas))
