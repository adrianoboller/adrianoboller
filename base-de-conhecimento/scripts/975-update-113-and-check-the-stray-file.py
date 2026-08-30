# Update #113 and check the stray file
# 29/08 01:14

import pathlib
p = pathlib.Path("docs/PENDENCIAS.md")
s = p.read_text()
s = s.replace("levou a inserção de **44,4 → 18,5 µs (2,40×)** e a carga em lote pela rede de **25.985 → 37.021 linhas/s**.",
              "levou a inserção de **44,4 → 18,5 µs (2,40×)**, e o cabeçalho que parou de reserializar o esquema por linha levou a **17,0 µs (2,61× no total)**; a carga em lote pela rede foi de **25.985 → 39.287 linhas/s**.")
p.write_text(s)
