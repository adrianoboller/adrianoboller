# Resolve the second conflict and regenerate
# 29/08 20:32

import io
p="phxsql/docs/PENDENCIAS.md"
s=io.open(p,encoding="utf-8").read()
i=s.index("<<<<<<< HEAD")
j=s.index("\n=======\n",i)
k=s.index(">>>>>>> worktree-agent-a66be917dc46200c2")
kfim=s.index("\n",k)+1
# fica o HEAD: o lado deles e a tabela VELHA, que a frente 24 ja corrigiu
s = s[:i] + s[i+len("<<<<<<< HEAD\n"):j+1] + s[kfim:]
assert "<<<<<<<" not in s and ">>>>>>>" not in s
io.open(p,"w",encoding="utf-8").write(s)
print("resolvido")
