# Resolve and run gates for the colours merge
# 29/08 23:30

import io
M=">>>>>>> worktree-agent-ab94006f33099d347"
p="phxsql/docs/PENDENCIAS.md"
s=io.open(p,encoding="utf-8").read()
a=s.index("<<<<<<< HEAD"); b=s.index("\n=======\n",a); c=s.index(M,b); cfim=s.index("\n",c)+1
meu=s[a+len("<<<<<<< HEAD\n"):b+1]; deles=s[b+len("\n=======\n"):c]
linhas=[l for l in deles.split("\n") if l.startswith("| ☑️ | 139 |")]
assert len(linhas)==1
nova=linhas[0].replace("| ☑️ | 139 |","| ☑️ | 141 |",1)
marca="<!-- pedidos:contagem:inicio -->"
meu=meu.replace(marca, nova+"\n\n"+marca, 1)
s=s[:a]+meu+s[cfim:]
assert "<<<<<<<" not in s
io.open(p,"w",encoding="utf-8").write(s)
print("cores viraram o pedido 141")
