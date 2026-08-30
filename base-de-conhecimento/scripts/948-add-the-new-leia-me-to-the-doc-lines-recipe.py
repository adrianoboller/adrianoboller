# Add the new LEIA-ME to the doc-lines recipe
# 29/08 00:39

import pathlib
p = pathlib.Path("docs/dossie/numeros-do-projeto.py")
s = p.read_text()
s = s.replace('''    "bancada/LEIA-ME.md", "bancada/replicacao/LEIA-ME.md",''',
              '''    "bancada/LEIA-ME.md", "bancada/replicacao/LEIA-ME.md",
    "bancada/carga/LEIA-ME.md",''', 1)
p.write_text(s)

p = pathlib.Path("docs/dossie/LEIA-ME.md")
s = p.read_text()
s = s.replace('''    bancada/LEIA-ME.md bancada/replicacao/LEIA-ME.md \\''',
              '''    bancada/LEIA-ME.md bancada/replicacao/LEIA-ME.md \\
    bancada/carga/LEIA-ME.md \\''', 1)
p.write_text(s)
print("ok")
