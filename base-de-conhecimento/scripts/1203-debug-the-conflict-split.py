# Debug the conflict split
# 29/08 20:31

import io
p="phxsql/docs/PENDENCIAS.md"
s=io.open(p,encoding="utf-8").read()
i=s.index("<<<<<<< HEAD")
j=s.index("\n=======\n",i)
k=s.index(">>>>>>> worktree-agent-a66be917dc46200c2")
meu   = s[i+len("<<<<<<< HEAD\n"):j+1]
deles = s[j+len("\n=======\n"):k]
print("marca em meu:", "<!-- pedidos:contagem:inicio -->" in meu)
print("linhas 133 em deles:", [l[:40] for l in deles.split("\n") if l.startswith("| ☑️ | 133")])
print("inicio de deles:", repr(deles[:120]))
