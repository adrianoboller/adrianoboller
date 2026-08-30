# List sections on both sides of the SEGURANCA conflict
# 29/08 21:03

import io
p="phxsql/docs/SEGURANCA.md"
s=io.open(p,encoding="utf-8").read()
a=s.index("<<<<<<< HEAD"); b=s.index("\n=======\n",a); c=s.index(">>>>>>> worktree-agent-a7b2760cf4c033d58",b)
meu=s[a+len("<<<<<<< HEAD\n"):b+1]; deles=s[b+len("\n=======\n"):c]
import re
print("HEAD secoes:", re.findall(r"^##+ .*", meu, re.M)[:12])
print()
print("DELES secoes:", re.findall(r"^##+ .*", deles, re.M)[:14])
