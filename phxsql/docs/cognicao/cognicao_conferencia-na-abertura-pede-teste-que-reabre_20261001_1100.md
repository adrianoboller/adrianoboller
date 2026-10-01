# Conferência na abertura pede teste que REABRE — e a faixa da `Sequence` não tinha nenhum

**Estado:** PENDENTE

- **Quando:** 2026-10-01, 11:00
- **Onde:** `crates/phxsql-store/src/reg.rs` (`proxima_sem_andar`,
  `conferir_faixa_da_sequencia`), pedido 290
- **Custo:** a metade de produção do 290 (o `inicio` pelo `config.json`) teria
  entrado em cima de um contador que impedia a segunda inserção

## O que aconteceu

Ligando `replicacao.inicio_da_sequencia` ao motor, o primeiro teste pelo
soquete caiu na **segunda** inserção: «numera a sequencia na faixa 0 de 2, e
este servidor esta declarado na faixa 1». A tabela tinha nascido nesta mesma
corrida, neste mesmo nó. O contador gravado depois de entregar `v` era `v + 1`
— fora da classe de resto que a abertura confere —, e o servidor reabre a
tabela a cada pedido.

## O que eu concluí primeiro, e estava errado

Que o vermelho era do meu teste: a faixa 7 deixada por um teste vizinho no
global do processo. Era metade da verdade — o vizinho existia e também
mascarava a guarda (uma das três quedas passou com o defeito reposto na
primeira prova) —, mas a recusa da segunda inserção vinha do motor, e
aparecia com o `inicio` em 0 também.

## O que a medição disse

`passo = 2`, faixa 0: entregou 2, gravou 3; a abertura seguinte recusa. O
teste de ponta a ponta que existia (`dois_nos_com_faixas_diferentes_nunca_repetem_numero`)
insere 200 linhas com a tabela **aberta** — nunca reabre, e por isso nunca
passou pela conferência que o `v + 1` quebrava. O parecer do DBA de 01/10
achou o mesmo pelo outro lado: a tabela já gravada assim ficava sem saída,
porque o remédio (`ajustar_sequencia`) exige abrir.

## A regra

Invariante conferido na ABERTURA se prova com um teste que grava, FECHA e
reabre — e a guarda de estado global do processo começa o teste na classe
oposta à esperada.

## Como está guardado hoje

`a_tabela_com_faixa_reabre_depois_de_numerar` (`carimbo-e-faixa.rs`, guarda
`faixa-sai-da-classe`) e `a_tabela_com_o_contador_defeituoso_volta_a_abrir_pelo_maior_gravado`
(`reconciliar-sequencia.rs`, guarda `faixa-sem-saida`). O buraco que fica: a
CLI, a FFI e o `phxsql-cmd` não leem `config.json` e abrem com `inicio = 0` —
tabela numerada na faixa 1 recusa abrir por eles.
