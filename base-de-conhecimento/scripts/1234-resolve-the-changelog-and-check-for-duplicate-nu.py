# Resolve the CHANGELOG and check for duplicate numbers
# 30/08 04:03

import io
M=">>>>>>> worktree-agent-aabd2a7e50d3335da"
p="phxsql/CHANGELOG.md"
s=io.open(p,encoding="utf-8").read()
n=0
while "<<<<<<< HEAD" in s:
    a=s.index("<<<<<<< HEAD"); b=s.index("\n=======\n",a); c=s.index(M,b); cfim=s.index("\n",c)+1
    meu=s[a+len("<<<<<<< HEAD\n"):b+1]; deles=s[b+len("\n=======\n"):c]
    s=s[:a]+meu+deles+s[cfim:]   # aditivo
    n+=1
io.open(p,"w",encoding="utf-8").write(s)
print("CHANGELOG:",n,"bloco(s), aditivo")
