# Fix the remaining stale numbers
# 29/08 01:11

import pathlib
p = pathlib.Path("docs/DESEMPENHO.md")
s = p.read_text()
s = s.replace("de 18,5 (26%).", "de 17,0 (28%).")
s = s.replace("dariam 4,1 µs de 18,5. O que custa nessas gravações é o **CRC** (4,8 µs), não a chamada",
              "dariam 4,0 µs de 17,0. O que custa nessas gravações é o **CRC** (4,8 µs), não a chamada")
s = s.replace('''| nada (hoje) | 18,5 | — |
| o segundo índice | 12,6 | 1,47× |
| os dois índices e a conferência | 6,7 | **2,76×** |''',
'''| nada (hoje) | 17,0 | — |
| o segundo índice | 11,2 | 1,52× |
| os dois índices e a conferência | 5,4 | **3,15×** |''')
s = s.replace("(Os mesmos números antes do cache de páginas eram 44,4 / 30,6 / 7,3 — ganho de\n1,45× e 6,1×. O cache já cobrou boa parte do que adiar o índice cobraria, e o\nteto de 6,1× virou 2,76×.)",
              "(Os mesmos números antes do cache de páginas eram 44,4 / 30,6 / 7,3 — ganho de\n1,45× e 6,1×. O cache já cobrou boa parte do que adiar o índice cobraria, e o\nteto de 6,1× virou 3,15×. E §4.2 mostra que esse teto **não se realiza** com o\n`reindexar` de hoje.)")
s = s.replace('''   2,34 µs de CRC — **4,8 µs de 18,5, ou 26%**. É o maior pedaço isolado que''',
'''   2,34 µs de CRC — **4,8 µs de 17,0, ou 28%**. É o maior pedaço isolado que''')
p.write_text(s)
print("ok")
