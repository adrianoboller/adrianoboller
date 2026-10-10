# Campo lido que o aviso de campo estranho chama de «ignorado»

**Estado:** FRUTÍFERO
**Evidência:** `crates/phxsql-server/src/config.rs::todo_campo_lido_da_protecao_e_conhecido`

## O que aconteceu

Montando a linha de `protecao.bloquear_por_codigo` na tela de configuração
(pedido 766, fatia P15), a lista `SECOES_CONHECIDAS["protecao"]` do
`config.rs` tinha cinco campos e o `Protecao::para_json` devolvia seis. O que
faltava era justamente o `bloquear_por_codigo`, entrado na P8: lido pelo
`Protecao::de_json`, devolvido no `config` — e listado em `estranhas`.

## O que eu concluí primeiro, e estava errado

Que era o mesmo defeito do `recursos.cache_paginas` (campo que a tela
anunciava e ninguém lia — «configuração que não é lida mente»). É o
**inverso**: aqui o campo é lido e vale, e quem mente é o aviso, dizendo ao
dono do banco que a guarda que ele ligou «foi ignorada». O conserto não é dar
um leitor; é ensinar a lista.

## O que a medição disse

`{"protecao":{"bloquear_por_codigo":true}}` → `chaves_estranhas` devolvia
`["protecao.bloquear_por_codigo"]`; com o nome na seção, `[]`. O teste monta o
pedido a partir do próprio `para_json`, então tirar o nome da seção o derruba
(RED medido em 10/10/2026, junto com o catálogo de guardas,
`p15-bloquear-por-codigo-estranho`).

## A regra

A lista do que se conhece sai do que se lê: quando uma seção ganha campo, a
prova tira a lista do `para_json` da seção, e não de uma cópia digitada.

## Como está guardado hoje

Só para a seção `protecao`. As outras seções de `SECOES_CONHECIDAS` continuam
conferidas campo a campo por testes pontuais (`recursos`, `alertas`) — o
alcance da regra é este, e estender a prova às outras seções é o buraco que
fica.
