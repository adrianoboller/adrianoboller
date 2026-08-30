# Write the SQL layer design doc
# 29/08 03:05

import pathlib
# o documento novo entra na receita de linhas de doc, como o LEIA-ME manda
p = pathlib.Path("docs/dossie/LEIA-ME.md")
s = p.read_text()
assert "docs/*.md" in s
p2 = pathlib.Path("docs/dossie/numeros-do-projeto.py")
print("docs/*.md ja cobre o SQL.md:", "docs/*.md" in s)
