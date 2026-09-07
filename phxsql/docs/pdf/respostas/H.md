# H) exemplo de uso systables e syscolumns

> Corrida em 2026-09-07T16:25:26Z UTC · commit `a56a165` · `target/release/phxsqld`
> · reproduzido por `python3 bancada/sql-exemplos/exercitar.py`

## Resposta curta

`systables`/`sistabelas` (sinônimos) devolvem uma linha por tabela do
database, com contagem de linhas/colunas/índices, a chave primária e quantas
chaves estrangeiras a tabela tem. `syscolumns`/`siscolunas` devolvem uma
linha por **coluna** de todas as tabelas (ou de uma só, com `"tabela":"..."`),
com tipo, tamanho, se é obrigatória/primária/estrangeira e em quais índices
entra. As duas respeitam a mesma permissão de leitura por tabela do resto do
protocolo (`pode_ver_tabela`) — uma tabela negada simplesmente não aparece. Há
ainda `catalogo`, o inventário das próprias operações do protocolo, com
detalhe por operação.

## Exemplo exercitado

Estado no momento da corrida: database `loja` com as tabelas `clientes` (7
linhas, sobra dos itens A/F) e `auditoria` (1 linha, criada no item F).

### `systables`

```json
{"database":"loja","total":2,"tabelas":[
  {"tabela":"auditoria","schema":"","registros":1,"slots":1,"colunas":3,
   "indices":0,"chave_primaria":null,"chaves_estrangeiras":0,
   "bytes_por_linha":130,"paginada":false,"particao":"","volumes":0},
  {"tabela":"clientes","schema":"","registros":7,"slots":7,"colunas":5,
   "indices":1,"chave_primaria":"porId","chaves_estrangeiras":0,
   "bytes_por_linha":98,"paginada":false,"particao":"","volumes":0}],
 "ok":true,"op":"systables","ms":0}
```

`sistabelas` (mesmo pedido, nome em português) devolve **byte a byte a mesma
lista**, só o campo `"op"` muda para `"sistabelas"`.

### `syscolumns` — todas as colunas de todas as tabelas do database

```json
{"database":"loja","total":8,"colunas":[
  {"tabela":"auditoria","posicao":1,"nome":"evento","tipo":"Str(120)",
   "tamanho":120,"obrigatoria":false,"primaria":false,"estrangeira":false,
   "nos_indices":[]},
  {"tabela":"auditoria","posicao":2,"nome":"softdeleted","caption":"Excluido",
   "tipo":"Bool","tamanho":1,"obrigatoria":true},
  {"tabela":"auditoria","posicao":3,"nome":"rownum","caption":"Nº","tipo":"UInt8",
   "tamanho":8,"obrigatoria":true},
  {"tabela":"clientes","posicao":1,"nome":"id","tipo":"Int8","tamanho":8,
   "obrigatoria":true,"primaria":true,"nos_indices":["porId"]},
  {"tabela":"clientes","posicao":2,"nome":"nome","tipo":"Str(40)","tamanho":40},
  {"tabela":"clientes","posicao":3,"nome":"cidade","tipo":"Str(40)","tamanho":40},
  {"tabela":"clientes","posicao":4,"nome":"softdeleted","tipo":"Bool","tamanho":1,"obrigatoria":true},
  {"tabela":"clientes","posicao":5,"nome":"rownum","tipo":"UInt8","tamanho":8,"obrigatoria":true}],
 "ok":true,"op":"syscolumns","ms":0}
```

`softdeleted` e `rownum` aparecem em **toda** tabela mesmo sem terem sido
declaradas — são colunas de sistema que o motor acrescenta sempre, com
`caption`/`descricao` explicando o papel (`"Marca a linha como excluida sem
apagar..."`, `"Ordem de chegada da linha. O motor preenche; nunca reaproveita
numero."`).

### `syscolumns` filtrado por tabela (`"tabela":"clientes"`)

```json
{"database":"loja","total":5,"colunas":[
  {"tabela":"clientes","posicao":1,"nome":"id","tipo":"Int8","primaria":true,"nos_indices":["porId"]},
  {"tabela":"clientes","posicao":2,"nome":"nome","tipo":"Str(40)"},
  {"tabela":"clientes","posicao":3,"nome":"cidade","tipo":"Str(40)"},
  {"tabela":"clientes","posicao":4,"nome":"softdeleted","tipo":"Bool"},
  {"tabela":"clientes","posicao":5,"nome":"rownum","tipo":"UInt8"}],
 "ok":true,"op":"syscolumns","ms":0}
```

Só as 5 colunas de `clientes` — `auditoria` some da lista.

### `catalogo` — o inventário das operações

Sem argumento, devolve o total visível para a sessão:

```json
{"total":123,"ocultas":0,"ok":true,"op":"catalogo","ms":0}
```

(123 operações no protocolo desta corrida, mais a lista `operacoes`, omitida
aqui por tamanho.)

Com `"operacao":"declarar_fk"`, devolve o **detalhe** — parâmetros, se
escreve, e um exemplo pronto:

```json
{"pedida":"declarar_fk","operacao":{
  "nome":"declarar_fk","apelidos":[],
  "resumo":"Declara uma chave estrangeira numa tabela que já existe -- declara, não impõe: o motor não a confere na gravação.",
  "permissao":"criar","escreve":true,
  "parametros":[
    {"nome":"database","tipo":"string","obrigatorio":true},
    {"nome":"tabela","tipo":"string","obrigatorio":true},
    {"nome":"nome","tipo":"string","obrigatorio":true},
    {"nome":"colunas","tipo":"array","obrigatorio":true},
    {"nome":"tabela_ref","tipo":"string","obrigatorio":true},
    {"nome":"colunas_ref","tipo":"array","obrigatorio":false},
    {"nome":"ao_excluir","tipo":"string","obrigatorio":false,
     "para_que":"SÓ restringir. Nunca se mata o pai que tem filhos: cascata, anular e nada não existem deste lado, e a recusa acontece na declaração e não na gravação"},
    {"nome":"ao_alterar","tipo":"string","obrigatorio":false,
     "para_que":"cascata (padrão), restringir, anular ou nada — as quatro acontecem na gravação desde a SP000057"}],
  "exemplo":"{\"op\":\"declarar_fk\",\"database\":\"loja\",\"tabela\":\"pedidos\",\"nome\":\"fk_cliente\",\"colunas\":[\"cliente_id\"],\"tabela_ref\":\"clientes\",\"colunas_ref\":[\"id\"]}"},
 "ok":true,"op":"catalogo","ms":0}
```

Com `"operacao":"xyzzy_nao_existe"` — não é um 404 seco, diz que a operação
não existe:

```json
{"pedida":"xyzzy_nao_existe","operacao":null,
 "motivo":"a operacao \"xyzzy_nao_existe\" nao existe",
 "ok":true,"op":"catalogo","ms":0}
```

*Nota (o resumo do `catalogo` já traz um número que a resposta desta rodada
contradiz em parte):* o texto de `ao_alterar` no catálogo diz que "as quatro
[ações] acontecem na gravação desde a SP000057", mas o item G desta mesma
bateria mostrou `ao_excluir:"cascata"` sendo **recusado na declaração**, não
na gravação — os dois lados da chave não têm o mesmo momento de recusa
(`ao_excluir` recusa cedo; `ao_alterar` aceita as quatro formas e decide na
gravação). Não é contradição do motor, é uma frase do catálogo que descreve
só o lado `ao_alterar` mas está no campo que a UI mostraria ao lado dos dois.

## O que NÃO existe, e é dispensa registrada

- **Filtro de `systables` por schema** — a operação lista todas as tabelas do
  database inteiro; separar por schema não foi pedido e não foi exercitado.
- **`syscolumns` com paginação** — devolve tudo de uma vez; para um database
  com milhares de colunas isso não foi medido nesta rodada (fora do escopo
  do pedido, que pede o exemplo de uso, não o custo).
- **Registro de tabela ilegível aparecendo como erro dentro da lista** —
  `docs/` já documenta que uma tabela corrompida vira uma linha com
  `"erro":"..."` em vez de derrubar `systables` inteiro; não reproduzido aqui
  porque exigiria corromper um arquivo de propósito, fora do escopo deste
  exemplo.

## Como se refaz

```bash
python3 bancada/sql-exemplos/exercitar.py
```

A função `item_h` roda as cinco chamadas (`systables`, `sistabelas`,
`syscolumns` sem filtro, `syscolumns` com `tabela`, `catalogo` nas três
formas) sobre o estado que os itens A e F já deixaram no database `loja`.
