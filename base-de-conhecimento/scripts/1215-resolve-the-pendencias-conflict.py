# Resolve the PENDENCIAS conflict
# 29/08 21:35

import io
M=">>>>>>> worktree-agent-aa0bb680877fcf680"
p="phxsql/docs/PENDENCIAS.md"
s=io.open(p,encoding="utf-8").read()
a=s.index("<<<<<<< HEAD"); b=s.index("\n=======\n",a); c=s.index(M,b); cfim=s.index("\n",c)+1
meu=s[a+len("<<<<<<< HEAD\n"):b+1]; deles=s[b+len("\n=======\n"):c]
linhas=[l for l in deles.split("\n") if l.startswith("| ☑️ | 135 |")]
assert len(linhas)==1, [l[:30] for l in deles.split("\n") if l.startswith("| ☑️")]
nova=linhas[0].replace("| ☑️ | 135 |","| ☑️ | 138 |",1)
marca="<!-- pedidos:contagem:inicio -->"
meu=meu.replace(marca, nova+"\n\n"+marca, 1)
s=s[:a]+meu+s[cfim:]
assert "<<<<<<<" not in s
io.open(p,"w",encoding="utf-8").write(s)
print("PENDENCIAS resolvido")
