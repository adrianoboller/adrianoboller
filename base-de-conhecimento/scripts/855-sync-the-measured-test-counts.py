# Sync the measured test counts
# 28/08 22:43

import pathlib, re
p = pathlib.Path("docs/dossie/dossie-phxsql-0.15.html")
s = p.read_text()
# o numero de testes tambem aparece na legenda final do video/capa
s = s.replace("42.790 linhas de Rust, zero dependências externas, 573 testes",
              "42.790 linhas de Rust, zero dependências externas, 574 testes")
p.write_text(s)
