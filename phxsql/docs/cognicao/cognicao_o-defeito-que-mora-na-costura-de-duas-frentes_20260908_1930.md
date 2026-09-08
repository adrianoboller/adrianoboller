# O defeito que mora na COSTURA: dois lados certos, resultado errado

Descoberto em 08/09/2026, por volta das 19:30, ao ligar a F-CONSULTA
(as ops `agrupar`/`consultar`/visões/`diferencas`) com a F-SQL (a gramática
que as produz).

## 1. O que aconteceu

Dois defeitos, dos dois lados do encontro, e nenhum deles aparecia na frente
que o causou.

**O primeiro.** `SELECT nome AS quem FROM v_todos` devolvia
`{"quem": null}` para **todas** as linhas — um `SELECT` que responde a
quantidade certa de linhas com uma coluna inteira de nulos.

A causa: a projeção acontecia **duas vezes**. `phxsql_sql::planejar_sobre`
produz um pedido `consultar` que já leva `colunas: [{"coluna":"nome",
"apelido":"quem"}]`, e o `op_consultar` projeta na fonte, pondo o **apelido**
como chave da linha. Depois, `resposta_do_sql` (servidor.rs) via
`Saida::Colunas([("nome","quem")])` no plano e projetava outra vez —
procurando `nome` numa linha que já se chamava `quem`. Não achava, e escrevia
`Json::Nulo` sob a chave `quem`.

**O segundo.** `cargo test --workspace` estava com **16 testes vermelhos** no
`phxsql-ffi`, todos com a mesma mensagem: `indice porId: 0 expressoes para 1
colunas`. O PSCH v9 (F-NÚCLEO) acrescentou a lista paralela `IndexDef.expressoes`
e ensinou `Schema::new` a exigir uma entrada por coluna; o conserto entrou no
`esquema_de_json` do servidor e **não** no irmão — o
`phx_esquema_indice_coluna` do FFI, que monta o `IndexDef` campo por campo, em
vez de pelo `IndexDef::new` que preenche as duas listas.

## 2. O que eu concluí primeiro, e estava errado

No primeiro, olhei o pedido e o achei correto (estava), olhei o `op_consultar`
e o achei correto (estava), e **concluí que o defeito era no `campo()` do
`consultar` — resolução de nome qualificado.** Cheguei a imprimir `escolhas` e
a primeira linha para provar: `[("nome","quem")]` e uma linha com `nome` lá
dentro. O `consultar` estava fazendo exatamente o certo, e o dado sumia
**depois** dele.

O erro de método foi olhar cada lado **isolado**, que é justamente o que os
testes de cada frente já faziam. O que quebrou o impasse foi imprimir a
RESPOSTA FINAL, e não os pedaços.

No segundo, a conclusão errada foi mais barata e mais perigosa: rodei
`cargo test -p phxsql-server` a corrida inteira, ele deu verde, e eu tratei
isso como «a suíte está verde». Rodar só a própria crate é medir só o que já
se sabe.

## 3. O que a medição disse

- **16** testes do `phxsql-ffi` vermelhos, e `cargo test -p phxsql-server`
  com **1.032 verdes** na mesma árvore. A crate que eu tocava não sabia de
  nada.
- **Zero** testes acusavam a projeção dupla antes do teste ponta a ponta: a
  F-SQL prova que o TEXTO vira o PEDIDO certo, e a F-CONSULTA prova que o
  PEDIDO devolve o DADO certo. Nenhuma das duas provas cruza o meio.
- O defeito da projeção **não é** um caso raro: ele atinge todo `SELECT` com
  `AS` que passe por `consultar` — visão, junção, `IN (SELECT …)`, janela.

## 4. A regra

**Frente que produz um contrato e frente que o consome fecham a costura com um
teste que atravessa as duas — e ele mede o DADO, não o pedido.** E antes de
declarar uma rodada pronta, rode `cargo test --workspace`: a suíte da própria
crate só prova o que a própria crate sabe.

## 5. Como está guardado hoje

- A costura: `mod testes_sql_composto` em `crates/phxsql-server/src/servidor.rs`
  — sete testes que entram pelo texto SQL e conferem o dado que sai (o PAR da
  junção, e não a contagem; o órfão do `LEFT`; o parâmetro hostil que continua
  sendo um valor). O cabeçalho do módulo diz por que eles não moram no crate
  de SQL.
- A projeção dupla: `resposta_do_sql` não reprojeta o que a op já projetou, com
  o comentário que conta o caso. Forçar `ja_projetou = false` derruba dois
  testes.
- A lista paralela do FFI: `phx_esquema_indice_coluna` empurra `None` em
  `expressoes` junto com a coluna, com o comentário nomeando o irmão.
- **Onde o buraco ficou:** não há guarda que impeça o *próximo* irmão de
  `IndexDef` montado campo por campo de esquecer a lista paralela. O conferidor
  genérico para isso está recusado pelo mesmo argumento das oito interpolações
  de erro cru (`CLAUDE.md`): casador de texto reprovaria construções legítimas.
  O que acha é procurar o irmão — e o que acusa, hoje, é o workspace inteiro
  verde.
