# Índice adiado: a marca no disco fecha a janela que o portão da carga não fecha

**Estado:** PENDENTE

## O que aconteceu

Pedido 324, 30/09/2026: `bulkinsert` com `"adiar_indice": true` suspende o
`.ndx` (byte 53, `NdxFile::suspender`) e o `bulkinsert(false)` o reconstrói
antes de soltar a reserva. O parecer do papel C pedia a R4 (pedido 322: o
Portão 4 olhar `juntar`/`unir`/`pivotar`/`diferencas`) como pré-requisito,
porque o lado B de uma junção lê a tabela reservada sem passar pelo portão.

## O que eu concluí primeiro, e estava errado

Que a R4 teria de entrar junto, e que o adiamento era seguro «porque a tabela
está reservada». As duas coisas vinham da mesma premissa: a de que quem
protege o índice vazio é a **reserva**. A reserva é memória de processo, a
própria ligação lê, e três operações escondem a tabela dela.

## O que a medição disse

`o_lado_b_da_juncao_nao_le_a_arvore_suspensa`: o `juntar` de outra ligação com
`b.tabela` = a tabela em carga **responde certo** (1 par, `lidas_b: 10`) — lê
o `.reg`, não a árvore. Toda pergunta à árvore suspensa (`buscar`, `verificar`,
a conferência de FK) recusa com `EM_CARGA` pelo `descritor`, de qualquer
ligação. O 322 continua aberto pelo que ele é (a reserva promete exclusividade
e o lado B a fura), mas deixou de ser o que impedia resposta **errada**.

E o número da carga, pelo motor e na forma que a R1 permite (dois índices não
únicos, tabela vazia, `--example indice-adiado-de-verdade`, máquina com carga
média de 3 a 7 em 4 núcleos): **1,62–1,76×** a um milhão de linhas; a cem mil
as faixas se cruzam. O 2,10× de 17/09 tinha um índice único no regime em
linha, que paga a pergunta «já existe?» por linha — e o índice único não se
adia.

## A regra

Garantia que depende de ninguém ler vai para o **dado**, não para a trava de
quem escreve: marque no arquivo o que a árvore não sabe responder, e deixe o
portão único do índice recusar por qualquer caminho.

## Como está guardado hoje

Guardas `suspensao-do-indice-so-na-ram`, `carga-adiada-solta-sem-reconstruir`
e `carga-adiada-orfa-sem-reconstruir` no catálogo; a queda por `SIGKILL` em
`crates/phxsql-store/tests/indice-adiado.rs`. O buraco que fica: o 322, a
leitura do `.reg` da tabela reservada pelo lado B.
