# Add manual section and update README
# 28/08 18:04

import io
p='README.md'
s=io.open(p,encoding='utf-8').read()
velho='''Motor de dados em Rust no modelo de arquivos separados do HFSQL(R): cada tabela
lógica é a soma de cinco arquivos físicos — mais um sexto, o espelho `.bkp`,
quando ele está ligado.

```
cadastroClientes.reg    registros, na ordem de digitação
cadastroClientes.ndx    índices (B+tree)
cadastroClientes.bin    binários
cadastroClientes.memo   textos longos
cadastroClientes.log    diário de inclusões, alterações e exclusões

.reg + .ndx + .bin + .memo + .log  =  cadastroClientes
```'''
novo='''Motor de dados em Rust no modelo de arquivos separados do HFSQL(R): cada tabela
lógica é a soma de sete arquivos físicos — mais um oitavo, o espelho `.bkp`,
quando ele está ligado.

```
cadastroClientes.reg    registros, na ordem de digitação
cadastroClientes.ndx    índices (B+tree)
cadastroClientes.bin    binários
cadastroClientes.memo   textos longos
cadastroClientes.log    diário de inclusões, alterações e exclusões
cadastroClientes.trash  as linhas que saíram do .reg, inteiras
cadastroClientes.reason por que cada linha foi excluída, e por quem

.reg + .ndx + .bin + .memo + .log + .trash + .reason = cadastroClientes
```

Os três últimos são **os arquivos do administrador**: o `.trash` guarda o dado
que alguém mandou apagar, e o `.reason` costuma ser mais revelador que o
registro que foi excluído.'''
assert velho in s
s=s.replace(velho,novo,1)

velho2='''## Por que cinco arquivos'''
novo2='''## Por que arquivos separados'''
assert velho2 in s
s=s.replace(velho2,novo2,1)

velho3='''- O **`.log`** é append-only e sem índice, então registrar uma operação custa
  36 bytes no fim de um arquivo — não atrapalha a escrita.'''
novo3='''- O **`.log`** é append-only e sem índice, então registrar uma operação custa
  36 bytes no fim de um arquivo — não atrapalha a escrita.
- O **`.trash`** e o **`.reason`** também são append-only, e existem porque uma
  exclusão precisa deixar rastro: a linha inteira num, o porquê no outro. O
  `.trash` é gravado e **sincronizado antes** de o slot do `.reg` ser liberado —
  entre perder o dado e duplicá-lo, o motor duplica.'''
assert velho3 in s
s=s.replace(velho3,novo3,1)
s=s.replace('''  phxsql-store/    os cinco arquivos, os volumes, a hierarquia e a tabela''',
            '''  phxsql-store/    os sete arquivos, os volumes, a hierarquia e a tabela''',1)
io.open(p,'w',encoding='utf-8').write(s)
print('readme ok')
