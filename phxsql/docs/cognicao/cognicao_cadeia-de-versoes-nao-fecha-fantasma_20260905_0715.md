# Cadeia de versões não fecha fantasma — a linha que nasceu depois não tem versão velha

**Descoberto em 05/09/2026, ~07:15**, cruzando os quatro fenômenos que a frente
ACID mediu (`docs/ACID.md` §4.1) com as dez divergências do desenho da Sombra
(`docs/PESQUISA-MVCC-E-FORMATO.md` §7), para responder ao pedido 179 **quais
deles a Sombra fecha**.

## 1. O que aconteceu

O pedido pedia a lista: dos quatro fenômenos medidos acontecendo — leitura não
repetível, fantasma, perda de atualização e *write skew* —, quais o desenho
fecha. Fui item a item, e o fantasma não fechou.

As dez divergências respondem, todas, **onde a versão velha mora**: fora do
`.reg`, em RAM, por delta, com o externo por conteúdo, resolvida na leitura por
marca monotônica. O fantasma pergunta outra coisa: *como a visão recusa ver uma
linha que **nasceu** depois dela?* Uma linha recém-inserida **não tem versão
velha** — a cadeia de sombra, por construção, não tem nada a dizer sobre ela.

Nenhuma das dez cobre isso, e a tabela de provas da §7.7 daquele documento
também não: ela tem o teste da leitura repetível e **não tem** o do fantasma.

## 2. O que eu concluí primeiro, e estava errado

Que a leitura repetível e o fantasma eram **o mesmo mecanismo com dois nomes** —
que quem congela a versão de uma linha congela junto o conjunto de linhas. É
como a norma os apresenta (dois degraus vizinhos: `REPEATABLE READ` e
`SERIALIZABLE`), e é como eu li a §4.3 do `CONCORRENCIA.md`, que fala de
«leitura repetível» sem separar as duas.

São mecanismos diferentes: um filtra **valor** por versão, o outro filtra
**existência** por nascimento. E a diferença tem consequência prática imediata:
implementar só o primeiro entrega **meia consistência**, que é pior que
nenhuma — uma varredura que esconde valor novo e mostra linha nova devolve um
estado que o banco **nunca teve**. A inteira o cliente sabe que não tem; a meia
ele descobre num relatório que não fecha.

## 3. O que a medição disse

Da corrida da frente ACID, que mede os dois separados e com o controle da mesma
corrida:

| fenômeno | medido | a cadeia de versões fecha? |
|---|---|---|
| leitura não repetível | duas leituras da mesma linha: **50** e depois **77** | **sim** |
| fantasma | a mesma varredura: **2** e depois **3** linhas | **não** — precisa de filtro de nascimento |

E o filtro de nascimento não pede formato novo, porque o número já está em
disco: a coluna de sistema **`rownum`** é um contador por tabela, atribuído na
gravação, que nunca reaproveita número (`crates/phxsql-core/src/schema.rs`,
`COLUNA_ROWNUM`; o contador vive nos bytes 92..100 do cabeçalho do volume 1,
`reg.rs`). Como nada vai a disco antes do `COMMIT`, a ordem do `rownum` **é** a
ordem de commit.

Três ressalvas conferidas no fonte, e a terceira é a armadilha:

* `Schema::coluna_rownum()` devolve **`None`** em tabela gravada antes da v5 do
  esquema;
* o `rownum` é **por tabela** — visão sobre duas tabelas precisa das duas marcas
  tomadas no mesmo instante, sob a trava;
* o **`rowid` não serve**: na partição alfanumérica ele não é monotônico no
  tempo (`docs/FORMATO.md` — *«a Silva digitada primeiro mora no `_S`, com rowid
  alto, e a Alves digitada depois mora no `_A`, com rowid 1»*). É exatamente a
  razão de o `rownum` existir.

## 4. A regra

**Quando um desenho responde «onde o dado velho mora», pergunte o que ele
responde sobre o dado que NÃO tem versão velha.** Cadeia de versões congela
valor; ela não congela existência — e conjunto congelado pela metade é um estado
que nunca existiu.

## 5. Como está guardado hoje

* Em `docs/SOMBRA.md` §1.2, com as três ressalvas e a consequência de ordem: se
  a Sombra for feita, **o filtro de nascimento se desenha antes da cadeia de
  versões**, porque é ele que decide se a visão é visão ou meia visão.
* Na §7 do mesmo documento entraram **duas** provas que faltavam: a do fantasma
  (com o defeito reposto devolvendo 2 e depois 3) e a de que **meia consistência
  não passa** — ligado só o filtro de nascimento, o teste tem de reprovar.
* Um ponteiro na §11.3 do `docs/CONCORRENCIA.md`, que é onde a próxima pessoa
  vai procurar.
* **Onde o buraco ficou:** a §7 da `PESQUISA-MVCC-E-FORMATO.md` continua com dez
  divergências e sem esta peça. Não a editei — aquele documento é o registro da
  pesquisa de 04/09, e reescrevê-lo apagaria a data em que o buraco existia.
