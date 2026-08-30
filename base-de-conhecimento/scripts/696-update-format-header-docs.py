# Update format header docs
# 28/08 19:05

import io
p='docs/FORMATO.md'
s=io.open(p,encoding='utf-8').read()
velho='''Uma tabela de dados do PhxSql é composta por sete arquivos físicos que
compartilham o mesmo nome-base — mais um oitavo, opcional:

```
cadastroClientes.reg + .ndx + .bin + .memo + .log + .trash + .reason
                     ( +  .bkp, o espelho, quando ligado )
```'''
novo='''Uma tabela de dados do PhxSql é composta por sete arquivos físicos que
compartilham o mesmo nome-base — mais o espelho e o descritor:

```
cadastroClientes.reg + .ndx + .bin + .memo + .log + .trash + .reason
                     ( +  .bkp, o espelho, quando ligado )
                     ( +  .pag, o descritor de partição )
```'''
assert velho in s
s=s.replace(velho,novo,1)
s=s.replace('''| `.reason` | Por que cada linha foi excluída, e por quem | `PHXRSN\\0\\0` | sim | **só `administrar`** |''',
            '''| `.reason` | Por que cada linha foi excluída, e por quem | `PHXRSN\\0\\0` | sim | **só `administrar`** |
| `.pag` | Descritor de partição, em JSON | — (texto) | não | quem lê a tabela |''',1)
s=s.replace('''| 92 | 32 | reservado |
| 124 | 4 | CRC-32 dos bytes 0..124 |''','''| 92 | 8 | `proximo_rownum` — próximo valor da coluna de sistema `rownum` (só o volume 1) |
| 100 | 8 | `slots_no_balde` — slots já usados **neste** volume (só na partição alfanumérica) |
| 108 | 16 | reservado |
| 124 | 4 | CRC-32 dos bytes 0..124 |''',1)
s=s.replace('| 8 | 2 | versão do formato (2) |','| 8 | 2 | versão do formato (3) |',1)
io.open(p,'w',encoding='utf-8').write(s)
print('ok')
