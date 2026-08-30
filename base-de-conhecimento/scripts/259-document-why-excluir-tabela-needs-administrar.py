# Document why excluir_tabela needs administrar
# 28/08 10:49

import pathlib
p = pathlib.Path('docs/USUARIOS.md')
s = p.read_text()
v = '''| `replicar` | `posicao`, `replicar` |

### Três regras que decidem tudo'''
n = '''| `replicar` | `posicao`, `replicar` |

> **Por que `excluir_tabela` pede `administrar` e não `excluir`.** Poder excluir
> uma *linha* não é poder excluir a *tabela*: a primeira operação perde um
> registro, a segunda apaga o `.reg`, o `.ndx`, o `.bin`, o `.memo`, o `.log` e
> o espelho de uma vez, com todos os volumes de cada um. Não há desfazer nem
> lixeira, então a permissão é a mais alta. O servidor ainda exige o nome da
> tabela repetido no campo `confirmar`.

### Três regras que decidem tudo'''
assert s.count(v) == 1
p.write_text(s.replace(v, n))
print('ok')
