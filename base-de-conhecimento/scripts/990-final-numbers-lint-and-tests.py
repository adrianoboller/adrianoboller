# Final numbers, lint and tests
# 29/08 01:58

import pathlib
p = pathlib.Path("docs/DESEMPENHO.md")
s = p.read_text()
s = s.replace("| Inserção local, 2 índices (`onde-doi`) | 22.516/s | 58.767/s | **2,61×** |",
              "| Inserção local, 2 índices (`onde-doi`) | 22.516/s | 62.763/s | **2,79×** |")
p.write_text(s)
