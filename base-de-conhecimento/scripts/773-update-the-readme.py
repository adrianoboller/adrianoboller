# Update the README
# 28/08 20:03

import pathlib
p = pathlib.Path("README.md")
s = p.read_text()
antigo = """## Paginação: o cursor sai de graça

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
que o motor preenche e nunca reaproveita."""
novo = """## Paginação: anda por cursor, salta por posição

Num motor relacional, pular para o meio de uma tabela grande exige um índice: a
ordem lógica não tem nada a ver com a posição física. Aqui tem —
`offset = data_offset + (rowid−1) × slot_size`. Continuar depois do rowid
500.000 **não é procurar: é uma conta.**

```json
{"op":"varrer","database":"loja","tabela":"clientes","max":200,"depois":4000}
{"op":"varrer","database":"loja","tabela":"clientes","max":200,"pular":100000}
```

Toda tabela tem a coluna de sistema **`rownum`** — a ordem de chegada da linha,
que o motor preenche e nunca reaproveita. Se ninguém apagou de vez e ninguém
marcou como excluída, a **posição** de uma linha na lista *é* o `rownum` dela
menos um — e aí «ir para a página 500» é uma bissecção de vinte leituras, e não
meio milhão de passos. O motor confere as duas condições no cabeçalho, em tempo
constante, e diz na resposta qual caminho pagou.

Medido numa tabela de 200.000 linhas, pelo protocolo, pedindo 200 linhas:

| `pular` | bissecção | passo |
|---:|---:|---:|
| 200 | 7 ms | 6 ms |
| 20.000 | 7 ms | 18 ms |
| 100.000 | 6 ms | 72 ms |
| 199.800 | 6 ms | **131 ms** |

A bissecção é **plana** — e os 6 ms dela são decodificar e serializar as 200
linhas, não achar o começo.

## Carga em lote

Gravar mil linhas com mil pedidos custa mil aberturas de tabela, mil travas e
mil `fsync`. `inserir_lote` faz tudo uma vez só — **2.715 → 25.985 linhas/s
(9,6×)**, medido com 20.000 linhas pela rede.

O mesmo pedido aceita texto colado em **JSON, CSV, TXT, XML ou HTML**, e
adivinha o formato pelo conteúdo. A primeira linha manda: as colunas casam pelo
**nome**, não pela posição."""
assert antigo in s
s = s.replace(antigo, novo)
p.write_text(s)
print("ok")
