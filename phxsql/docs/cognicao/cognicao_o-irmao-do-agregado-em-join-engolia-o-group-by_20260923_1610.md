# O irmão do agregado-sobre-junção engolia o `GROUP BY` inteiro, e devolvia `Ok`

## 1. O que aconteceu

Consertando o pedido 394 (agregado na projeção de uma consulta composta vira
coluna fantasma), medi as portas VIZINHAS antes de escrever o terceiro braço.
Três delas tinham o mesmo defeito por outro caminho, e **uma era pior que o
pedido**:

- `SELECT v.nome FROM p JOIN v ON v.id=p.vid WHERE p.z = 1 GROUP BY v.nome`
  devolvia **`Ok`** de `analisar_comando`, com a cláusula inteira viajando
  DENTRO do texto do filtro: `onde: Some(Expressao("p.z = 1 GROUP BY v.nome"))`.
  A causa é `capturar_ate_clausula`, que não parava em `GROUP`/`HAVING` nem no
  `ON` (`condicao_de_juncao`) nem no `WHERE` (`onde_composta`).
- `ORDER BY SUM(v)`, `GROUP BY UPPER(c)` e `PARTITION BY MAX(x)` liam a palavra
  da função como NOME DE COLUNA — a mesma coluna fantasma do 394, nas três
  listas que chamam `identificador()` (`uma_ordenacao_restrita`,
  `lista_de_ordenacoes`, `lista_de_colunas_do_group_by`).

## 2. O que eu concluí primeiro, e estava errado

Concluí, lendo o `Ok` acima, que o `GROUP BY` engolido era **resposta errada
calada** — dado errado chegando ao cliente, o pior caso da capa da crate.
Medi antes de escrever isso no relatório: `Expressao::analisar("p.z = 1 GROUP
BY v.nome")` **recusa**, com `sobrou GROUP depois do fim da expressao`. Ou
seja, não há dado errado: há uma recusa que chega **na camada errada, na hora
errada e com o nome errado** — o `analisar_comando` aprova, o driver e o
desenhador de consulta acham que a frase é válida, e só na execução o
avaliador de expressão reclama de uma coisa que não é o problema.

Errei também a saída: o comentário de `planejar_sobre` recomendava
`SELECT COUNT(*) FROM (SELECT * FROM v_c) AS x` para agregar sobre visão. Não
existe — medido, essa frase cai na gramática composta e recusa pelo 394. Eu ia
copiar essa saída para dentro da mensagem nova.

## 3. O que a medição disse

- Recusa do pedido 394, colada: `esquema invalido: SQL, coluna 11: esperava
  FROM, e veio "("` — a mesma para junção, subconsulta no `FROM`, `WITH` e
  `IN (SELECT …)` (4 portas, 4 mensagens idênticas).
- `GROUP BY`/`HAVING` na composta: `Ok`, com a cláusula dentro da `expressao`.
  Com **uma** igualdade no `ON` a recusa saía como «ON sem nenhuma igualdade
  de colunas» — mentira: a igualdade estava escrita ali, e o que faltava era a
  parada.
- Substrato: **não existe, e é de contrato.** A op `agrupar` declara `TAB`
  (`catalogo.rs`) e abre a tabela por `abrir_travada`; ela não recebe
  sub-pedido. A op `consultar` recebe `de`/`juntar`/`existe`/`escalar` e
  **nenhum** campo de agregação. Não há meia tradução possível.
- 10 testes novos; com o defeito reposto, **6 falham** e os 2 de comportamento
  velho (pedido serializado byte a byte) passam nos dois estados.

## 4. A regra

**Cláusula que a captura não conhece não é cláusula ausente: é cláusula
ENGOLIDA.** Quando uma gramática captura tokens «até a próxima palavra de
parada», a lista de paradas é um INVENTÁRIO — o que falta nela não recusa, vira
texto de outra cláusula e a recusa sai numa camada onde ninguém vai procurá-la.

## 5. Como está guardado hoje — e onde o buraco ficou

Guardado em `crates/phxsql-sql`: `recusa_de_funcao_composta` (o terceiro braço
de `item_de_projecao_composta`), `recusa_de_agregacao_composta` (um lugar só,
em `consulta_apos_select`, porque os dois caminhos que engoliam a cláusula
desaguam nele) e `Analisador::recusar_funcao_no_lugar_de_coluna` (o crivo único
das três listas de nome de coluna). As três recusas citam «Pedido 394».

O buraco: **o substrato continua não existindo**, e agora ele recusa dizendo
isso em vez de mentir. Quem for dá-lo muda o contrato de uma das duas ops —
`agrupar` passando a aceitar `de` (sub-pedido) como o `consultar` já aceita, ou
`consultar` ganhando `por`/`agregados`. É decisão do dono e trabalho de
servidor, não desta camada. E fica um aviso para quem mexer: `ORDER BY` sobre
o resultado agregado depende do apelido, então a ordem de `agrupar` e a de
`consultar` não são a mesma pergunta.
