# Update the format header docs
# 28/08 19:05

import io
p='docs/FORMATO.md'
s=io.open(p,encoding='utf-8').read()

# cabecalho: o oitavo/nono arquivo
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

velho2='''| `.reason` | Por que cada linha foi excluída, e por quem | `PHXRSN\\0\\0` | sim | **só `administrar`** |'''
novo2='''| `.reason` | Por que cada linha foi excluída, e por quem | `PHXRSN\\0\\0` | sim | **só `administrar`** |
| `.pag` | Descritor de partição, em JSON | — (texto) | não | quem lê a tabela |'''
assert velho2 in s
s=s.replace(velho2,novo2,1)

# cabecalho do .reg: os campos novos
velho3='''| 92 | 32 | reservado |
| 124 | 4 | CRC-32 dos bytes 0..124 |'''
novo3='''| 92 | 8 | `proximo_rownum` — próximo valor da coluna de sistema `rownum` (só o volume 1) |
| 100 | 8 | `slots_no_balde` — slots já usados **neste** volume (só na partição alfanumérica) |
| 108 | 16 | reservado |
| 124 | 4 | CRC-32 dos bytes 0..124 |'''
assert velho3 in s
s=s.replace(velho3,novo3,1)

s=s.replace('''| `.reg` | Registros, na ordem de digitação | `PHXREG\\0\\0` | sim | quem tem `ler` |''',
            '''| `.reg` | Registros, na ordem de digitação | `PHXREG\\0\\0` | sim | quem tem `ler` |''',1)
io.open(p,'w',encoding='utf-8').write(s)
