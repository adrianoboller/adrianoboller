# Resolve SEGURANCA and find stale section references
# 29/08 21:03

import io,re
p="phxsql/docs/SEGURANCA.md"
s=io.open(p,encoding="utf-8").read()
a=s.index("<<<<<<< HEAD"); b=s.index("\n=======\n",a); c=s.index(">>>>>>> worktree-agent-a7b2760cf4c033d58",b)
cfim=s.index("\n",c)+1
meu=s[a+len("<<<<<<< HEAD\n"):b+1]; deles=s[b+len("\n=======\n"):c]
# o Profiler fica com a §10 (chegou antes); a cifra vira §11
deles = re.sub(r"^## 10\. ", "## 11. ", deles, flags=re.M)
deles = re.sub(r"^### 10\.(\d+)", lambda m: f"### 11.{m.group(1)}", deles, flags=re.M)
s = s[:a] + meu.rstrip("\n") + "\n\n---\n\n" + deles.lstrip("\n") + s[cfim:]
assert "<<<<<<<" not in s
io.open(p,"w",encoding="utf-8").write(s)
print("SEGURANCA resolvido")
