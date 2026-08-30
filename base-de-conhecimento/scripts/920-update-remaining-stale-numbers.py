# Update remaining stale numbers
# 29/08 00:21

import pathlib
p = pathlib.Path("docs/DESEMPENHO.md")
s = p.read_text()

s = s.replace('''| Se sair do caminho crítico | µs por linha | ganho |
|---|---:|---:|
| nada (hoje) | 44,4 | — |
| o segundo índice | 30,6 | 1,45× |
| os dois índices e a conferência | 7,3 | **6,1×** |''',
'''| Se sair do caminho crítico | µs por linha | ganho |
|---|---:|---:|
| nada (hoje) | 18,5 | — |
| o segundo índice | 12,6 | 1,47× |
| os dois índices e a conferência | 6,7 | **2,76×** |

(Os mesmos números antes do cache de páginas eram 44,4 / 30,6 / 7,3 — ganho de
1,45× e 6,1×. O cache já cobrou boa parte do que adiar o índice cobraria, e o
teto de 6,1× virou 2,76×.)''', 1)

s = s.replace('''| 9 | Buffers grandes em vez de escritas pequenas | Escreve por slot; o `strace` conta 41 chamadas por linha | **Vale medir** — mas 98% de CPU e 0,0 MiB lidos dizem que o disco não é quem espera |''',
'''| 9 | Buffers grandes em vez de escritas pequenas | Escreve por slot; são 2,06 páginas de `.ndx` gravadas por linha, medidas | **Medido, e é pequeno.** Um `lseek` custa 0,10 µs: mesmo 41 chamadas por linha dariam 4,1 µs de 18,5. O que custa nessas gravações é o **CRC** (4,8 µs), não a chamada |''', 1)

s = s.replace('''| Inserção pela rede, linha a linha vs. lote | 2.715/s | 25.985/s | **9,6×** |''',
'''| Inserção pela rede, linha a linha vs. lote | 2.609/s | 37.021/s | **14,2×** |
| Inserção local, 2 índices (`onde-doi`) | 22.516/s | 53.988/s | **2,40×** |''', 1)
p.write_text(s)
print("ok")
