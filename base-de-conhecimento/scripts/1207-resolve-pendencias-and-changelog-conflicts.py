# Resolve PENDENCIAS and CHANGELOG conflicts
# 29/08 20:52

import io
M=">>>>>>> worktree-agent-a0bc9d47d803ae652"

p="phxsql/docs/PENDENCIAS.md"
s=io.open(p,encoding="utf-8").read()
a=s.index("<<<<<<< HEAD"); b=s.index("\n=======\n",a); c=s.index(M,b); cfim=s.index("\n",c)+1
meu=s[a+len("<<<<<<< HEAD\n"):b+1]; deles=s[b+len("\n=======\n"):c]
linha=[l for l in deles.split("\n") if l.startswith("| ☑️ | 133 |")][0]
nova=linha.replace("| ☑️ | 133 |","| ☑️ | 135 |",1)
marca="<!-- pedidos:contagem:inicio -->"
meu=meu.replace(marca, nova+"\n\n"+marca, 1)
s=s[:a]+meu+s[cfim:]
assert "<<<<<<<" not in s
io.open(p,"w",encoding="utf-8").write(s)

p="phxsql/CHANGELOG.md"
s=io.open(p,encoding="utf-8").read()
n=0
while "<<<<<<< HEAD" in s:
    a=s.index("<<<<<<< HEAD"); b=s.index("\n=======\n",a); c=s.index(M,b); cfim=s.index("\n",c)+1
    meu=s[a+len("<<<<<<< HEAD\n"):b+1]; deles=s[b+len("\n=======\n"):c]
    s=s[:a]+meu+deles+s[cfim:]   # aditivo: as duas secoes entram
    n+=1
io.open(p,"w",encoding="utf-8").write(s)
print("PENDENCIAS + CHANGELOG resolvidos,", n, "blocos")
