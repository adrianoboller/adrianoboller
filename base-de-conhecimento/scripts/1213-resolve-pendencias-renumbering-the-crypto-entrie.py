# Resolve PENDENCIAS renumbering the crypto entries
# 29/08 21:03

import io
M=">>>>>>> worktree-agent-a7b2760cf4c033d58"
p="phxsql/docs/PENDENCIAS.md"
s=io.open(p,encoding="utf-8").read()
a=s.index("<<<<<<< HEAD"); b=s.index("\n=======\n",a); c=s.index(M,b); cfim=s.index("\n",c)+1
meu=s[a+len("<<<<<<< HEAD\n"):b+1]; deles=s[b+len("\n=======\n"):c]
# os deles numeraram 133 e 134, que ja sao do Profiler e do restaurar -> 136 e 137
novas=[]
for l in deles.split("\n"):
    if l.startswith("| ☑️ | 133 |"): novas.append(l.replace("| ☑️ | 133 |","| ☑️ | 136 |",1))
    elif l.startswith("| ☑️ | 134 |"): novas.append(l.replace("| ☑️ | 134 |","| ☑️ | 137 |",1))
marca="<!-- pedidos:contagem:inicio -->"
meu=meu.replace(marca, "\n".join(novas)+"\n\n"+marca, 1)
s=s[:a]+meu+s[cfim:]
assert "<<<<<<<" not in s
io.open(p,"w",encoding="utf-8").write(s)
print("PENDENCIAS resolvido")
