# Resolve additive conflicts keeping both sides
# 29/08 17:10

import re, pathlib
# Conflitos aditivos: cada lado acrescentou peca propria no mesmo ponto.
# A resolucao correta e ficar com os DOIS lados, na ordem HEAD depois ramo.
alvos = ["phxsql/crates/phxsql-core/src/error.rs",
         "phxsql/crates/phxsql-server/src/servidor.rs"]
padrao = re.compile(r"<<<<<<< [^\n]*\n(.*?)=======\n(.*?)>>>>>>> [^\n]*\n", re.S)
for a in alvos:
    p = pathlib.Path(a); t = p.read_text()
    novo, n = padrao.subn(lambda m: m.group(1) + m.group(2), t)
    p.write_text(novo)
    print(f"{a}: {n} conflito(s) resolvido(s) mantendo os dois lados")
