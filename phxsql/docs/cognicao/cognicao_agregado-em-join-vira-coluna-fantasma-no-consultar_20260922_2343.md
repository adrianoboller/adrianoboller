# Agregado dentro de um SELECT com JOIN vira coluna fantasma no `consultar`

## 1. O que aconteceu

Medindo a premissa do pedido 305 (papel E, 22/09/2026) — passar oito consultas
do parecer `docs/propostas/phoenix-query-designer-2026-09-17.md` por
`phxsql_sql::analisar` — quatro delas combinavam agregado (`SUM`/`COUNT`) com
`JOIN`, porque e assim que o mockup do Query Designer e o `docs/SQL.md` §1.2
do proprio parecer descrevem a consulta canonica: `SELECT col, SUM(x) AS a
FROM t JOIN … ON … WHERE … GROUP BY … ORDER BY … LIMIT n`.

As quatro recusaram, e as quatro com a MESMA mensagem generica: `esquema
invalido: SQL, coluna N: esperava FROM, e veio "("`. Isolando o caso minimo:

```
SELECT SUM(p.valor) AS valor, v.nome, r.nome
FROM pedidos p
JOIN vendedores v ON v.id = p.vendedor_id
...
```

recusa em `phxsql_sql::analisar_comando` na coluna 11 — bem em cima do `(` de
`SUM(`. A causa esta em `crates/phxsql-sql/src/consulta.rs`,
`item_de_projecao_composta` (linha ~998): quando o item da projecao de um
`SELECT` que vira `consultar` (qualquer coisa com `JOIN`, subconsulta no
`FROM`, `WITH` ou `IN (SELECT …)`) e uma chamada de funcao, o codigo so trata
`ROW_NUMBER() OVER (...)` e "outra `FUNCAO(...) OVER (...)`" (que vira uma
recusa nomeada, "nao tem substrato"). Para `SUM(`/`COUNT(`/`AVG(` **sem**
`OVER` depois, nenhum dos dois `if` bate, e o codigo cai no caminho de
fallback — `let nome = self.identificador("nome de coluna")?` — que le a
PALAVRA `SUM` como se fosse um NOME DE COLUNA valido (o token e um
`Token::Palavra`, e `identificador()` nao distingue "palavra que e nome de
funcao" de "palavra que e nome de coluna"). O parser aceita `SUM` como coluna,
nao ve `.` nem `AS` a seguir (o proximo token e `(`), fecha o item e devolve.
De volta em `projecao_composta`, o proximo token nao e virgula, entao a lista
para ali — com UM item fantasma chamado `"SUM"`. Quem chamou espera `FROM`
a seguir e encontra o `(` que sobrou do agregado: dai a mensagem, que nao
fala nem de agregado nem de JOIN.

## 2. O que eu concluí primeiro, e estava errado

Antes de isolar o caso minimo, a hipotese natural — e a que eu quase escrevi
no relatorio sem checar — foi "`SUM(p.valor)` recusa porque a coluna dentro do
agregado esta QUALIFICADA (`p.valor`), e o parser do agregado so aceita nome
simples". Testei essa hipotese sozinha, SEM `JOIN` (`SELECT SUM(p.valor) AS
valor FROM pedidos p`), e ela recusa de verdade — mas com uma mensagem
DIFERENTE e mais precisa: `esperava ) do agregado, e veio "."` (coluna 13,
exatamente no ponto). Sao DOIS defeitos diferentes que eu tinha fundido num
so: (a) `chamada_de_agregado` (o SELECT simples, sem JOIN) nao aceita
`tabela.coluna` dentro do agregado — so nome simples; (b) `consulta.rs` (o
SELECT com JOIN) nem chega a reconhecer que `SUM(` e um agregado, e o defeito
aparece MESMO com coluna simples dentro do agregado (`SUM(valor)` teria o
mesmo destino se estivesse num `SELECT … JOIN …`, porque a causa e o `if`
que so trata `ROW_NUMBER`/`... OVER`, nao o parenteses qualificado). A
qualificacao do parecer ("SIM, inteiro" para o padrao inteiro
`SELECT col, SUM(x) AS a FROM t JOIN …`) tambem estava errada, mas por
LEITURA — ninguem tinha rodado esse padrao especifico pela crate ainda.

## 3. O que a medição disse

Numero medido, nao citado: das 8 consultas do pedido 305 (5 do mockup + 3 da
captura de tela), **0/8 passam** por `phxsql_sql::analisar` OU por
`phxsql_sql::analisar_comando` — o mesmo placar pelas duas portas, porque
nenhuma delas chega perto de ser um `Comando::Consulta` valido antes de trombar
num dos gaps documentados (`DISTINCT`+JOIN, `\|\|`, sub-select na projecao,
`UNION`) ou neste, que nao estava documentado como recusa nomeada: agregado
sem `OVER` dentro de uma projecao COMPOSTA. Isolado: `SELECT SUM(p.valor) AS
valor, v.nome FROM pedidos p JOIN vendedores v ON v.id=p.vendedor_id` recusa
com a mensagem generica de roteamento, nao com uma recusa que nomeie
"agregado", "GROUP BY" ou "JOIN".

## 4. A regra

**Toda "porta de entrada" de uma projecao (a do `SELECT` simples E a do
`consultar` composto) tem de reconhecer a MESMA lista de nomes-de-funcao antes
de decidir se aceita ou recusa — reconhecer so `ROW_NUMBER` e deixar as
outras cairem no caminho de "e coluna" produz uma coluna fantasma com o nome
da funcao, e o erro aparece LONGE de onde o problema esta.** Quando um
"builder" novo (como o Query Designer do pedido 305) for gerar consulta
composta com agregado, ele tem de saber que **hoje isso nao tem substrato
nenhum na camada SQL** (nem recusa nomeada — recusa com o nome errado), e nao
so "GROUP BY sobre visao nao agrupa" (que e o unico caso ja documentado,
em `consulta.rs:493-496`, e so cobre a VIEW, nao o JOIN direto).

## 5. Como está guardado hoje — e onde o buraco ficou

Guardado: a recusa "COUNT(*)/GROUP BY sobre visao nao tem substrato" existe e
tem teste (`consulta.rs`, `planejar_sobre_count_ou_group_by_recusa`), mas so
cobre o caminho de consultar uma VIEW ja materializada como fonte. O caminho
GERAL — `SELECT … agregado … FROM t JOIN …` direto, sem visao no meio — NAO
tem essa recusa nomeada: cai no fallback de "e coluna" de
`item_de_projecao_composta` e produz a mensagem de roteamento errada. Não há
teste que trave isto (nenhum teste de `consulta.rs` passa `SUM(`/`COUNT(`
sem `OVER` na projecao de uma consulta com `JOIN`). Fica para quem tocar
`item_de_projecao_composta` a seguir — nomear a recusa (reconhecer
`FuncaoAgregada::de_nome_de_funcao` ali tambem, e recusar dizendo "agregado
sobre junção ainda não tem substrato — componha por fora" em vez de deixar
cair no caminho de coluna) é o conserto mínimo; dar substrato de verdade ao
agregado sobre `consultar` é outra rodada, e é motor (papel B/C), não tela.
