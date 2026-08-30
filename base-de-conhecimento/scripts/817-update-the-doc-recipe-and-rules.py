# Update the doc recipe and rules
# 28/08 20:38

import pathlib
p = pathlib.Path("docs/dossie/LEIA-ME.md")
s = p.read_text()
s = s.replace("""`dossie-phxsql.html` é o fonte da página publicada em:

**https://claude.ai/code/artifact/5c14044e-0dc5-4832-b015-224ab1e40033**""",
"""`dossie-phxsql-0.15.html` é o fonte da página publicada em:

**https://claude.ai/code/artifact/5c14044e-0dc5-4832-b015-224ab1e40033**

O nome mudou na 0.15.0 — o dossiê foi refeito com o estado medido daquela
versão. Os dois scripts abaixo aceitam o caminho do HTML como argumento, para
que trocar o nome de novo não exija editá-los.""")
s = s.replace("""cat docs/*.md README.md CHANGELOG.md MANUAL.txt \\
    bancada/LEIA-ME.md marca/LEIA-ME.md docs/dossie/LEIA-ME.md \\
  | wc -l                                                          # linhas de doc""",
"""cat docs/*.md README.md CHANGELOG.md MANUAL.txt \\
    bancada/LEIA-ME.md bancada/replicacao/LEIA-ME.md \\
    marca/LEIA-ME.md docs/dossie/LEIA-ME.md \\
  | wc -l                                                          # linhas de doc""")
p.write_text(s)
