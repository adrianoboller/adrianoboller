# Update the DESEMPENHO tables
# 29/08 01:57

import pathlib, re
p = pathlib.Path("docs/DESEMPENHO.md")
s = p.read_text()

# a tabela das tres colunas ganha a quarta
alvo = '''| Esquema | antes | + cache de páginas | + cabeçalho enxuto | ganho |
|---|---:|---:|---:|---:|
| só `.reg` (sem índice nenhum) | 7,3 µs | 6,7 µs | **5,4 µs** | 1,35× |
| + 1 índice comum | 21,5 µs | 12,2 µs | **10,9 µs** | 1,97× |
| + o mesmo índice, agora único | 30,6 µs | 12,6 µs | **11,2 µs** | 2,73× |
| + 2 índices (a forma da bancada) | 44,4 µs | 18,5 µs | **17,0 µs** | **2,61×** |

| Parcela | antes | % | agora | % |
|---|---:|---:|---:|---:|
| `.reg` + `.log` | 7,3 | 16,5% | 5,4 | 31,8% |
| <span>↳ só o `.log` (§2.2)</span> | — | — | 1,22 | 7,2% |
| primeiro índice | 14,2 | 32,0% | 5,4 | 31,8% |
| conferir a chave única | 9,1 | 20,5% | 0,7 | **4,0%** |
| segundo índice | 13,8 | 31,0% | 5,5 | 32,4% |
| **total** | **44,4** | 100% | **17,0** | 100% |'''
novo = '''| Esquema | antes | + cache de páginas | + cabeçalho do `.reg` | + cabeçalho do `.log` | ganho |
|---|---:|---:|---:|---:|---:|
| só `.reg` (sem índice nenhum) | 7,3 µs | 6,7 µs | 5,4 µs | **4,8 µs** | 1,52× |
| + 1 índice comum | 21,5 µs | 12,2 µs | 10,9 µs | **10,2 µs** | 2,11× |
| + o mesmo índice, agora único | 30,6 µs | 12,6 µs | 11,2 µs | **10,5 µs** | 2,91× |
| + 2 índices (a forma da bancada) | 44,4 µs | 18,5 µs | 17,0 µs | **15,9 µs** | **2,79×** |

| Parcela | antes | % | agora | % |
|---|---:|---:|---:|---:|
| `.reg` + `.log` | 7,3 | 16,5% | 4,8 | 30,3% |
| <span>↳ só o `.log` (§2.2)</span> | — | — | 0,67 | 4,2% |
| primeiro índice | 14,2 | 32,0% | 5,4 | 33,9% |
| conferir a chave única | 9,1 | 20,5% | 0,3 | **1,9%** |
| segundo índice | 13,8 | 31,0% | 5,4 | 34,0% |
| **total** | **44,4** | 100% | **15,9** | 100% |'''
assert s.count(alvo) == 1
s = s.replace(alvo, novo, 1)
p.write_text(s)
print("tabela ok")
