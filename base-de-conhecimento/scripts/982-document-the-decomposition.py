# Document the decomposition
# 29/08 01:47

import pathlib
p = pathlib.Path("CHANGELOG.md")
s = p.read_text()
alvo = '''- **`--example indice-adiado`**'''
novo = '''- **`--example custo-do-log`**, que decompõe o bloco `.reg` + `.log` que este
  documento registrava como não decomposto. O diário custa **1,22 µs por
  evento (7,2% de uma inserção)**, ou 2,24 com a imagem da linha; a reescrita
  do cabeçalho, sozinha, custa 0,41. Responde «dá para guardar o diário em
  memória?»: segurar os eventos compraria 7,2% e trocaria uma garantia
  irreconstruível; parar de reescrever o cabeçalho compra 2,4% sem buffer
  nenhum.

- **`--example indice-adiado`**'''
assert s.count(alvo) == 1
s = s.replace(alvo, novo, 1)
p.write_text(s)
