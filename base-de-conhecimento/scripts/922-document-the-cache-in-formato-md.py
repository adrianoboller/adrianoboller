# Document the cache in FORMATO.md
# 29/08 00:22

import pathlib
p = pathlib.Path("docs/FORMATO.md")
s = p.read_text()
alvo = '''| 28 | 4 | CRC-32 da página, calculado sobre 0..28 e 32..fim |
| 32 | … | entradas |

### Chave completa'''
novo = '''| 28 | 4 | CRC-32 da página, calculado sobre 0..28 e 32..fim |
| 32 | … | entradas |

### As páginas quentes ficam em RAM

Não é formato — o arquivo é o mesmo com ou sem isto —, mas muda tanto o custo
de uma inserção que vale estar aqui do lado do CRC que o motivou.

Toda inserção **desce** a árvore: raiz, nó interno, folha. Sem cache, isso é um
`pread` de página inteira mais um CRC-32 de página inteira em cada nível, e a
raiz é a mesma página em todas as inserções da carga. Medido: **10,86 páginas
tocadas por linha**, das quais 8,80 são releituras, e o CRC de uma página de
4 KiB custa 2,34 µs. Eram **25,4 µs de CRC por linha**, de 44,4 µs medidos no
total.

Cada `.ndx` aberto guarda até 2.048 páginas (8 MiB), com despejo por segunda
chance. A gravação **atravessa sempre** para o arquivo: segurar página suja em
RAM daria mais e trocaria uma garantia por desempenho sem avisar — hoje só uma
queda da máquina atrasa o `.ndx` em relação ao `.reg`, e não uma queda do
processo.

Custo de uma inserção com dois índices: **44,4 → 18,5 µs**. Os números e a
varredura que escolheu o teto estão em `docs/DESEMPENHO.md`.

### Chave completa'''
assert s.count(alvo) == 1
s = s.replace(alvo, novo, 1)
p.write_text(s)
print("ok")
