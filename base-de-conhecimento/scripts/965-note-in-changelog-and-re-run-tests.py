# Note in CHANGELOG and re-run tests
# 29/08 00:57

import pathlib
p = pathlib.Path("CHANGELOG.md")
s = p.read_text()
alvo = '''- **`--example ordem-da-chave`**, que mede quanto a ordem das chaves custa. Foi
  ele que reprovou a hipótese do pedido 113 antes de ela virar código.'''
novo = '''- **`--example ordem-da-chave`**, que mede quanto a ordem das chaves custa. Foi
  ele que reprovou a hipótese do pedido 113 antes de ela virar código.

- **`--example indice-adiado`**, que responde «e se o `.ndx` parasse durante a
  carga e fosse reconstruído no fim?» com a reconstrução **dentro da conta**:
  **1,02×**. O `reindexar` de hoje insere chave a chave — uma descida por
  chave, o mesmo trabalho do caminho de dentro, feito depois. O ganho está na
  **construção em lote** da B+tree (varrer, ordenar, encher as folhas em
  sequência), cujo piso medido é 0,24 s contra os 2,54 s que o `reindexar`
  cobra. A ordem de trabalho é a inversa da intuição: o lote primeiro, o
  adiamento depois. Está em `docs/DESEMPENHO.md` §4.2.'''
assert s.count(alvo) == 1
s = s.replace(alvo, novo, 1)
p.write_text(s)
print("ok")
