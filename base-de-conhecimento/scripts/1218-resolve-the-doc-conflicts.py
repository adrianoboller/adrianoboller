# Resolve the doc conflicts
# 29/08 22:15

import io
M=">>>>>>> worktree-agent-aa230d13b205c7056"
p="phxsql/docs/PENDENCIAS.md"
s=io.open(p,encoding="utf-8").read()
n=0
while "<<<<<<< HEAD" in s:
    a=s.index("<<<<<<< HEAD"); b=s.index("\n=======\n",a); c=s.index(M,b); cfim=s.index("\n",c)+1
    meu=s[a+len("<<<<<<< HEAD\n"):b+1]; deles=s[b+len("\n=======\n"):c]
    # o lado deles e a lista VELHA de "planejados" ou a contagem digitada; fica o HEAD
    s=s[:a]+meu+s[cfim:]
    n+=1
io.open(p,"w",encoding="utf-8").write(s)
print("PENDENCIAS:",n,"blocos, ficou o HEAD")
