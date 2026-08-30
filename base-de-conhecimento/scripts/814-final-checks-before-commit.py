# Final checks before commit
# 28/08 20:36

import pathlib
p = pathlib.Path("README.md")
s = p.read_text()
s = s.replace("| `.log` — diário datado de inclusão, alteração e exclusão | pronto |",
              "| `.log` — diário datado de inclusão, alteração e exclusão, com a imagem da linha para replicar | pronto |")
p.write_text(s)
