# Update manual and README
# 28/08 19:07

import io
p='README.md'
s=io.open(p,encoding='utf-8').read()
velho='''.reg + .ndx + .bin + .memo + .log + .trash + .reason = cadastroClientes
```'''
novo='''.reg + .ndx + .bin + .memo + .log + .trash + .reason = cadastroClientes
```

Mais o `.pag` ao lado — um JSON que descreve como a tabela está partida, para
quem está de fora ler sem abrir o `.reg`. É gerado, e o motor nunca o lê.'''
assert velho in s
s=s.replace(velho,novo,1)

velho2='''- O **`.trash`** e o **`.reason`** também são append-only, e existem porque uma
  exclusão precisa deixar rastro: a linha inteira num, o porquê no outro. O
  `.trash` é gravado e **sincronizado antes** de o slot do `.reg` ser liberado —
  entre perder o dado e duplicá-lo, o motor duplica.'''
novo2='''- O **`.trash`** e o **`.reason`** também são append-only, e existem porque uma
  exclusão precisa deixar rastro: a linha inteira num, o porquê no outro. O
  `.trash` é gravado e **sincronizado antes** de o slot do `.reg` ser liberado —
  entre perder o dado e duplicá-lo, o motor duplica.

## Paginação: o cursor sai de graça

Num motor relacional, pular para o meio de uma tabela grande exige um índice: a
ordem lógica não tem nada a ver com a posição física. Aqui tem —
`offset = data_offset + (rowid−1) × slot_size`. Continuar depois do rowid
500.000 **não é procurar: é uma conta.**

```json
{"op":"varrer","database":"loja","tabela":"clientes","max":200,"depois":4000}
```

A página custa o tamanho dela, e não o da tabela. Medido no navegador com
20.000 linhas: **4,0 ms** por página pelo cursor, sem crescer com a
profundidade; **16,1 ms** por posição no mesmo ponto, e crescendo. `pular`
continua existindo como modo de compatibilidade.

Toda tabela tem a coluna de sistema **`rownum`** — a ordem de chegada da linha,
que o motor preenche e nunca reaproveita.

## Partição alfanumérica

Um arquivo por letra inicial de uma coluna:

```
cadastroClientes_A.reg  …  cadastroClientes_Z.reg
cadastroClientes_0.reg  …  cadastroClientes_9.reg
cadastroClientes_Outros.reg
```

São 37 volumes fixos, e o rowid é atribuído como
`(balde − 1) × registros_por_arquivo + slot` — a inversa exata da conta de
sempre, então **nenhum caminho de leitura mudou**. O teto passa a ser por letra,
e a ordem de digitação sai do `rowid` e vai para o `rownum`.'''
assert velho2 in s
s=s.replace(velho2,novo2,1)
io.open(p,'w',encoding='utf-8').write(s)
print('readme ok')
