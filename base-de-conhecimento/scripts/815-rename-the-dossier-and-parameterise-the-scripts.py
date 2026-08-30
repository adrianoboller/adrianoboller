# Rename the dossier and parameterise the scripts
# 28/08 20:37

import pathlib
for f in ["docs/dossie/numeros-do-projeto.py", "docs/dossie/numeros-da-bancada.py"]:
    p = pathlib.Path(f)
    s = p.read_text()
    s = s.replace('DOSSIE = RAIZ / "docs" / "dossie" / "dossie-phxsql.html"',
'''# Qual dossie reescrever. O nome mudou na 0.15.0 e pode mudar de novo:
# passar o caminho como primeiro argumento evita editar o script a cada vez.
def _alvo():
    for a in sys.argv[1:]:
        if a.endswith(".html"):
            return pathlib.Path(a)
    return RAIZ / "docs" / "dossie" / "dossie-phxsql-0.15.html"


DOSSIE = _alvo()''')
    if "import sys" not in s:
        s = s.replace("import pathlib", "import pathlib\nimport sys", 1)
    if "import pathlib" not in s:
        s = s.replace("import sys", "import pathlib\nimport sys", 1)
    p.write_text(s)
    print(f, "ok")
