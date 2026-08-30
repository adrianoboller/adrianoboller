# Resolve conflicts and regenerate the count
# 29/08 20:25

import io
p="phxsql/docs/PENDENCIAS.md"
s=io.open(p,encoding="utf-8").read()
i=s.index("<<<<<<< HEAD")
j=s.index("=======",i)
k=s.index(">>>>>>> worktree-agent-a754d6f644b4674d5")
# fica o lado do HEAD: a contagem sai do gerador, nao do teclado
s = s[:i] + s[i+len("<<<<<<< HEAD\n"):j] + s[k+len(">>>>>>> worktree-agent-a754d6f644b4674d5\n"):]
assert "<<<<<<<" not in s and ">>>>>>>" not in s
io.open(p,"w",encoding="utf-8").write(s)
print("PENDENCIAS resolvido")
