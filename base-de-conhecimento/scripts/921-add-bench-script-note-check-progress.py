# Add bench-script note; check progress
# 29/08 00:22

import pathlib
p = pathlib.Path("docs/DESEMPENHO.md")
s = p.read_text()
s = s.replace('''| Replicação | não existia | 4.273 eventos/s por réplica | — |

---

## 7.''','''| Replicação | não existia | 4.273 eventos/s por réplica | — |

A carga pela rede agora tem script: `bancada/carga/medir.py`. O número anterior
(2.715 → 25.985 linhas/s) foi medido **à mão**, sem programa que o refizesse —
e o motor mudou desde então. Os dois lados batem no linha a linha (2.715 e
2.609), que é o controle; o lote subiu de 25.985 para 37.021 por causa do cache
de páginas.

---

## 7.''', 1)
p.write_text(s)
print("ok")
