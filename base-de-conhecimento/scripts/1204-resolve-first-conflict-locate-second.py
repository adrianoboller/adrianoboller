# Resolve first conflict, locate second
# 29/08 20:31

import io
p="phxsql/docs/PENDENCIAS.md"
s=io.open(p,encoding="utf-8").read()
i=s.index("<<<<<<< HEAD")
j=s.index("\n=======\n",i)
k=s.index(">>>>>>> worktree-agent-a66be917dc46200c2")
kfim=s.index("\n",k)+1
meu   = s[i+len("<<<<<<< HEAD\n"):j+1]
deles = s[j+len("\n=======\n"):k]
linha = [l for l in deles.split("\n") if l.startswith("| ☑️ | 133 |")][0]
nova  = linha.replace("| ☑️ | 133 |","| ☑️ | 134 |",1)
marca = "<!-- pedidos:contagem:inicio -->"
meu = meu.replace(marca, nova + "\n\n" + marca, 1)
s = s[:i] + meu + s[kfim:]
io.open(p,"w",encoding="utf-8").write(s)
print("resolvido 1; restam:", s.count("<<<<<<<"))
