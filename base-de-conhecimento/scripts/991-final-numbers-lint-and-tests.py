# Final numbers, lint and tests
# 29/08 01:58

import pathlib
p = pathlib.Path("docs/PENDENCIAS.md")
s = p.read_text()
s = s.replace("e o cabeçalho que parou de reserializar o esquema por linha levou a **17,0 µs (2,61× no total)**",
              "o cabeçalho que parou de reserializar o esquema por linha levou a 17,0 e o cabeçalho do diário levou a **15,9 µs (2,79× no total)**")
p.write_text(s)
