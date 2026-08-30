# Commit the documentation and CLAUDE.md
# 28/08 23:10

import pathlib
p = pathlib.Path("docs/dossie/dossie-phxsql-0.15.html")
s = p.read_text()
s = s.replace("42.790 linhas de Rust, zero dependências externas, 574 testes",
              "43.800 linhas de Rust, zero dependências externas, 587 testes")
p.write_text(s)
