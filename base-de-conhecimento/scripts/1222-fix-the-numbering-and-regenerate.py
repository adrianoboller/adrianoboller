# Fix the numbering and regenerate
# 29/08 23:33

import io
p="phxsql/docs/PENDENCIAS.md"
s=io.open(p,encoding="utf-8").read()
# 1) tira os marcadores que sobraram (o lado do HEAD fica; o dos deles ja entrou como 139)
if "<<<<<<< HEAD" in s:
    a=s.index("<<<<<<< HEAD"); b=s.index("\n=======\n",a)
    c=s.index(">>>>>>> worktree-agent-ab94006f33099d347",b); cfim=s.index("\n",c)+1
    meu=s[a+len("<<<<<<< HEAD\n"):b+1]
    s=s[:a]+meu+s[cfim:]
# 2) o 139 das cores vira 141 (o 139 e da tela larga)
linhas=s.split("\n")
for i,l in enumerate(linhas):
    if l.startswith("| ☑️ | 139 | **Permitir mudar as cores"):
        linhas[i]=l.replace("| ☑️ | 139 |","| ☑️ | 141 |",1)
s="\n".join(linhas)
assert "<<<<<<<" not in s
io.open(p,"w",encoding="utf-8").write(s)
