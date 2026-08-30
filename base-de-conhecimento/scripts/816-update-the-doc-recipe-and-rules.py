# Update the doc recipe and rules
# 28/08 20:38

import pathlib
p = pathlib.Path("docs/dossie/numeros-do-projeto.py")
s = p.read_text()
s = s.replace('''DOCS_AVULSOS = (
    "README.md", "CHANGELOG.md", "MANUAL.txt",
    "bancada/LEIA-ME.md", "marca/LEIA-ME.md", "docs/dossie/LEIA-ME.md",
)''', '''DOCS_AVULSOS = (
    "README.md", "CHANGELOG.md", "MANUAL.txt",
    "bancada/LEIA-ME.md", "bancada/replicacao/LEIA-ME.md",
    "marca/LEIA-ME.md", "docs/dossie/LEIA-ME.md",
)''')
p.write_text(s)
