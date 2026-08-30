# Resolve the FORMATO conflict and renumber
# 29/08 20:31

import io
p="phxsql/docs/FORMATO.md"
s=io.open(p,encoding="utf-8").read()
i=s.index("<<<<<<< HEAD")
j=s.index("=======",i)
k=s.index(">>>>>>> worktree-agent-a66be917dc46200c2")
kfim=s.index("\n",k)+1
deles=s[j+len("=======\n"):k]
# o lado deles termina com o cabecalho "## 9. Hierarquia", que aqui ja e o 10
deles=deles.replace("## 8b. `backup.json`","## 10. `backup.json`")
deles=deles.rstrip("\n")
assert deles.endswith("## 9. Hierarquia: database, schema e tabela"), deles[-80:]
deles=deles[:-len("## 9. Hierarquia: database, schema e tabela")].rstrip("\n")
novo = deles + "\n\n## 11. Hierarquia: database, schema e tabela\n"
s = s[:i] + novo + s[kfim:]
assert "<<<<<<<" not in s and ">>>>>>>" not in s
# renumera o que vinha depois (11..15 -> 12..16)
for velho,nv in [("## 11. Reindex","## 12. Reindex"),
                 ("## 12. Identificadores","## 13. Identificadores"),
                 ("## 13. Limites","## 14. Limites"),
                 ("## 14. `gatilhos.json`","## 15. `gatilhos.json`"),
                 ("## 15. O que este formato","## 16. O que este formato")]:
    assert s.count(velho)==1, velho
    s=s.replace(velho,nv)
io.open(p,"w",encoding="utf-8").write(s)
