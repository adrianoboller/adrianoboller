# Make conferir importable without all four up
# 28/08 20:24

import pathlib
p = pathlib.Path("/tmp/claude-0/-home-user-adrianoboller/34595649-0af6-575a-8f79-80dbe8cb7a5d/scratchpad/rep/conferir.py")
s = p.read_text()
s = s.replace("CONEXOES = {n: liga(p) for n, p in PORTAS.items()}",
"""def conectar_todos():
    return {n: liga(p) for n, p in PORTAS.items()}

CONEXOES = {}
if __name__ == "__main__":
    CONEXOES = conectar_todos()""")
s = s.replace("""def posicoes():
    out = {}
    for n, fala in CONEXOES.items():""", """def posicoes():
    out = {}
    for n, fala in CONEXOES.items():""")
p.write_text(s)
