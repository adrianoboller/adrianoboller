# Update remaining numbers and the CHANGELOG
# 29/08 01:58

import pathlib
p = pathlib.Path("docs/DESEMPENHO.md")
s = p.read_text()
s = s.replace('''> cabeçalho que reserializava o esquema a cada linha (§2.0) levou a **17,0 µs**
> — **2,61× no total**.''',
'''> cabeçalho que reserializava o esquema a cada linha (§2.0) levou a 17,0, e o
> cabeçalho do diário que ia a disco a cada evento (§2.2) levou a **15,9 µs** —
> **2,79× no total**.''')
s = s.replace("de 17,0 (28%).", "de 15,9 (30%).")
s = s.replace("dariam 4,0 µs de 17,0. O que custa nessas gravações é o **CRC** (4,8 µs), não a chamada",
              "dariam 4,0 µs de 15,9. O que custa nessas gravações é o **CRC** (4,8 µs), não a chamada")
s = s.replace('''| nada (hoje) | 17,0 | — |
| o segundo índice | 11,2 | 1,52× |
| os dois índices e a conferência | 5,4 | **3,15×** |''',
'''| nada (hoje) | 15,9 | — |
| o segundo índice | 10,5 | 1,51× |
| os dois índices e a conferência | 4,8 | **3,31×** |''')
s = s.replace('''   2,34 µs de CRC — **4,8 µs de 17,0, ou 28%**. É o maior pedaço isolado que''',
'''   2,34 µs de CRC — **4,8 µs de 15,9, ou 30%**. É o maior pedaço isolado que''')
p.write_text(s)
print("ok")
