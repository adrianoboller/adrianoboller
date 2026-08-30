# Resolve the first PENDENCIAS conflict, renumbering restore to 134
# 29/08 20:31

import io,re
p="phxsql/docs/PENDENCIAS.md"
s=io.open(p,encoding="utf-8").read()
i=s.index("<<<<<<< HEAD")
j=s.index("\n=======\n",i)
k=s.index(">>>>>>> worktree-agent-a66be917dc46200c2")
kfim=s.index("\n",k)+1
meu   = s[i+len("<<<<<<< HEAD\n"):j+1]
deles = s[j+len("\n=======\n"):k]
# a linha 133 deles e o restaurar; o 133 ja e do profiler -> vira 134
linha = [l for l in deles.split("\n") if l.startswith("| ☑️ | 133 |")]
assert len(linha)==1
nova = linha[0].replace("| ☑️ | 133 |","| ☑️ | 134 |",1)
# entra logo depois do 133 do profiler, antes do bloco da contagem
marca = "\n<!-- pedidos:contagem:inicio -->"
assert marca in meu
meu = meu.replace(marca, "\n" + nova + "\n" + marca, 1)
s = s[:i] + meu + s[kfim:]
assert "<<<<<<< HEAD" not in s
io.open(p,"w",encoding="utf-8").write(s)
print("primeiro conflito resolvido")
