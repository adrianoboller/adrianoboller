# Renumber the Docker requests
# 30/08 04:09

import io,re
M=">>>>>>> worktree-agent-ac436726d4652b6e4"
p="phxsql/docs/PENDENCIAS.md"
s=io.open(p,encoding="utf-8").read()
a=s.index("<<<<<<< HEAD"); b=s.index("\n=======\n",a); c=s.index(M,b); cfim=s.index("\n",c)+1
meu=s[a+len("<<<<<<< HEAD\n"):b+1]; deles=s[b+len("\n=======\n"):c]
usados={int(m.group(1)) for m in re.finditer(r"^\| [☑◐]\S* \| (\d+) \|", s, re.M)}
prox=max(usados)+1
novas=[]
for l in deles.split("\n"):
    m=re.match(r"^(\| [☑◐]\S* \| )(\d+)( \|)", l)
    if m:
        novas.append(f"{m.group(1)}{prox}{m.group(3)}{l[m.end():]}"); print("pedido", m.group(2), "->", prox); prox+=1
marca="<!-- pedidos:contagem:inicio -->"
meu=meu.replace(marca, "\n".join(novas)+"\n\n"+marca, 1)
s=s[:a]+meu+s[cfim:]
assert "<<<<<<<" not in s
io.open(p,"w",encoding="utf-8").write(s)
