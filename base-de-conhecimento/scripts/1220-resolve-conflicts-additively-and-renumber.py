# Resolve conflicts additively and renumber
# 29/08 23:23

import io
M=">>>>>>> worktree-agent-ada20d79e1d8d591f"
p="phxsql/crates/phxsql-server/ui/index.html"
s=io.open(p,encoding="utf-8").read()
a=s.index("<<<<<<< HEAD"); b=s.index("\n=======\n",a); c=s.index(M,b); cfim=s.index("\n",c)+1
meu=s[a+len("<<<<<<< HEAD\n"):b+1]; deles=s[b+len("\n=======\n"):c]
s=s[:a]+meu+deles+s[cfim:]   # aditivo: o grafico mede a largura E a aba pausa o relogio
assert "<<<<<<<" not in s
io.open(p,"w",encoding="utf-8").write(s)
print("index.html resolvido (aditivo)")

M2=M
p="phxsql/docs/PENDENCIAS.md"
s=io.open(p,encoding="utf-8").read()
a=s.index("<<<<<<< HEAD"); b=s.index("\n=======\n",a); c=s.index(M2,b); cfim=s.index("\n",c)+1
meu=s[a+len("<<<<<<< HEAD\n"):b+1]; deles=s[b+len("\n=======\n"):c]
linhas=[l for l in deles.split("\n") if l.startswith("| ☑️ | 139 |") or l.startswith("| ◐ | 139 |")]
assert len(linhas)==1, [l[:22] for l in deles.split("\n") if l.startswith("| ")]
nova=linhas[0].replace("| 139 |","| 140 |",1)
marca="<!-- pedidos:contagem:inicio -->"
meu=meu.replace(marca, nova+"\n\n"+marca, 1)
s=s[:a]+meu+s[cfim:]
assert "<<<<<<<" not in s
io.open(p,"w",encoding="utf-8").write(s)
print("PENDENCIAS resolvido: o multitela virou 140")
