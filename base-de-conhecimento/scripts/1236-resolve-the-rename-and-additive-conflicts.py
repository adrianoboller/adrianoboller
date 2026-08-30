# Resolve the rename and additive conflicts
# 30/08 04:09

import io
M=">>>>>>> worktree-agent-ac436726d4652b6e4"
for p in ["phxsql/CHANGELOG.md","phxsql/docs/dossie/LEIA-ME.md"]:
    s=io.open(p,encoding="utf-8").read(); n=0
    while "<<<<<<< HEAD" in s:
        a=s.index("<<<<<<< HEAD"); b=s.index("\n=======\n",a); c=s.index(M,b); cfim=s.index("\n",c)+1
        meu=s[a+len("<<<<<<< HEAD\n"):b+1]; deles=s[b+len("\n=======\n"):c]
        s=s[:a]+meu+deles+s[cfim:]; n+=1
    io.open(p,"w",encoding="utf-8").write(s); print(p,n,"bloco(s) aditivo")
