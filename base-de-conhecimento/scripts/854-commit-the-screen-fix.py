# Commit the screen fix
# 28/08 22:35

import pathlib
p = pathlib.Path("CHANGELOG.md")
s = p.read_text()
antigo = """- **A tela da Replicação dizia que a replicação não existia.**"""
novo = """- **A tela da Replicação lia o campo errado da resposta de `bancos`.** A
  operação responde uma **lista** de nomes, não um objeto com `bancos` dentro;
  ler o campo devolvia vazio, e a tela dizia «nenhuma tabela ainda» numa
  réplica que tinha a tabela na árvore ao lado. Achado no mesmo vídeo, no
  quadro seguinte.

- **A tela da Replicação dizia que a replicação não existia.**"""
assert antigo in s
s = s.replace(antigo, novo)
p.write_text(s)
