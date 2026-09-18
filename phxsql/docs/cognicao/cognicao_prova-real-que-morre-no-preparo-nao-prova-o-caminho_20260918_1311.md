# Prova real que morre no preparo não prova o caminho que importa

Pedido 358 (o oráculo do rowid). Descoberto em 18/09/2026, 13:11 UTC.

## 1. O que aconteceu

O contrato do pedido 358 tinha um item duro: a recusa de «partição por posição
sobre coluna marcada» **não** podia morar dentro de `Schema::com_paginacao`,
porque `Schema::com_coluna` — o `ALTER TABLE ADD COLUMN` —
a chama de novo (`crates/phxsql-core/src/schema.rs:1283`) com as colunas que já
carregam a marca. Uma guarda ali derrubaria o `ADD COLUMN` de uma tabela que já
está em produção.

Escrevi a prova real desse item em
`crates/phxsql-store/tests/alfanumerica.rs:531`: montar a tabela com a
combinação, gravar, reabrir, ler, gravar e **acrescentar coluna**. Repus o
defeito (a guarda dentro do `com_paginacao`) e o teste ficou vermelho.

Só que ele ficou vermelho na linha do `com_paginacao` do
*preparo* — não na linha do `acrescentar_coluna`. O teste acusou, mas acusou
outra coisa: acusou que a tabela não podia mais nascer, e nunca chegou a
exercitar o `ADD COLUMN`.

## 2. O que eu concluí primeiro, e estava errado

Concluí que o teste estava provando o item 2 do contrato, porque ficou vermelho
com o defeito reposto e verde com o conserto — que é a forma da prova real.

Estava errado. **A prova real tem endereço, não só cor.** Um teste que morre no
preparo dá o mesmo vermelho de um teste que morre no caminho que importa, e a
diferença não aparece no `test result: FAILED`. Se um dia o `com_paginacao`
passasse a recusar apenas na criação e não na recolocação da paginação — que é
exatamente o desenho que o `ADD COLUMN` precisaria —, aquele teste continuaria
verde e o `ADD COLUMN` continuaria quebrado.

E houve um segundo engano, do mesmo naipe, meia hora antes: montei a `Paginacao`
do teste do núcleo com um literal (`max_arquivos: 9`) sobre o modo `PorLetra`.
O teste ficou vermelho, e eu li o vermelho como «a guarda pegou». Não era: era
`«a particao alfanumerica tem exatamente 37 volumes (A-Z, 0-9 e Outros); o
esquema pede 9»`. Vermelho pelo motivo errado, com a mesma cor do certo.

## 3. O que a medição disse

O conserto foi montar o esquema do teste pelo caminho que **não** passa pela
função sob suspeita: `Schema::do_disco(...).com_paginacao_do_disco(...)`, em
`crates/phxsql-core/src/schema.rs:3077`. Com isso o defeito reposto derruba o
teste em `schema.rs:3119` — a linha do `com_coluna`, com a mensagem da recusa do
358 — e não no preparo.

Medido, com o defeito reposto:

- bateria `acrescentar-coluna` inteira (20 testes): **20 passaram, 0 falharam**.
  Nenhuma tabela dela tem a combinação, então o `ADD COLUMN` quebrado não
  aparece em lugar nenhum da suíte que já existia.
- `alfanumerica::tabela_gravada_com_a_combinacao_...`: vermelho **no preparo**
  (na montagem do esquema, hoje na linha 550).
- `schema::testes_oraculo_do_rowid::a_tabela_que_ja_tem_a_combinacao_continua_ganhando_coluna`:
  vermelho **na linha do `com_coluna`** (linha 3119, o `expect` do `ADD COLUMN`).

Ou seja: das três, só a terceira prova o item. As outras duas dizem «alguma
coisa quebrou».

## 4. A regra

**Prova real diz em que LINHA ficou vermelha, e essa linha tem de ser a do
caminho sob prova — não a do preparo.** Quando o preparo do teste chama a mesma
função que o defeito habita, monte o preparo por outro caminho (aqui, o do
disco), ou o teste vai morrer antes de chegar onde interessa.

## 5. Como está guardado hoje

Guardado em três testes, e cada um com um alcance escrito no doc:

- `crates/phxsql-core/src/schema.rs:3105` —
  `a_tabela_que_ja_tem_a_combinacao_continua_ganhando_coluna`, com o preparo
  pelo caminho do disco e o comentário dizendo **por que** ele não usa o
  `com_paginacao`.
- `crates/phxsql-store/tests/alfanumerica.rs:531` — o irmão em disco, que prova
  o resto (abrir, ler, gravar, `ADD COLUMN` de verdade pelo `Table`). Ele morre
  no preparo com o defeito reposto, e isso agora está dito.
- `crates/phxsql-core/src/schema.rs:3077` — a função auxiliar
  `com_a_combinacao`, com o comentário do segundo engano: a paginação sai do
  construtor de cada modo, nunca de um literal, porque um número na mão faz o
  teste falhar pelo motivo errado.

**O buraco que ficou:** não há conferidor que ache, no repositório, teste cujo
preparo chama a função sob prova. Não tentei escrever um — seria um casador de
texto sobre nome de função, e a lei dos 8 erros crus já mediu o que esse tipo
de conferidor entrega. Por ora é leitura, e é isto aqui que a lembra.
