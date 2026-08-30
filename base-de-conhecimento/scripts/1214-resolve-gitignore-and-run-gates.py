# Resolve gitignore and run gates
# 29/08 21:10

import io
p="phxsql/.gitignore"
s=io.open(p,encoding="utf-8").read()
a=s.index("<<<<<<< HEAD"); b=s.index("\n=======\n",a); c=s.index(">>>>>>> worktree-agent-ab71188a46c854aab",b); cfim=s.index("\n",c)+1
meu=s[a+len("<<<<<<< HEAD\n"):b+1]; deles=s[b+len("\n=======\n"):c]
s=s[:a]+meu+"\n"+deles+s[cfim:]
assert "<<<<<<<" not in s
io.open(p,"w",encoding="utf-8").write(s)
print("ok")
