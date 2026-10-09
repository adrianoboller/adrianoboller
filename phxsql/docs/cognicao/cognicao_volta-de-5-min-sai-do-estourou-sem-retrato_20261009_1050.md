# A volta de 5 minutos sai do `estourou`, sem a linha `retrato`

**Estado:** PENDENTE

- **Quando:** 2026-10-09, 10:50
- **Onde:** `crates/phxsql-server/ui/aquario.js` (`voltar5min`, `tanqueEm`),
  `crates/phxsql-server/src/aquario/log.rs` (`consultar`, a `faixa` na leitura)
- **Pedido:** 707, fatia A13

## O que aconteceu

O desenho (§4.3) manda a volta de 5 minutos achar o último `retrato` anterior à
janela e dobrar os eventos por cima. Lido o `log.rs` na base `117a7667`: as
linhas `retrato` e `nasceu` **não são gravadas** por ninguém (o próprio
cabeçalho do arquivo diz «o que AINDA NAO grava»). Só `estourou`, `mudou` e
`contagem` chegam ao disco.

## O que eu concluí primeiro, e estava errado

Que a volta de 5 minutos estava bloqueada até alguém gravar o `retrato` de 30 s
— e que a fatia A13 teria de abrir esse escritor no servidor, com o custo de uma
linha a cada 30 s com tarefa viva.

## O que a medição disse

A linha `estourou` traz o **fim** (`quando_ms`) e a **duração** (`ms`). Isso
basta para saber quem estava no tanque em qualquer instante `t` da janela:
`fim - ms <= t < fim`. As tarefas ainda vivas vêm do último retrato da tela
(`agora_ms - ms`). Na prova `testes-web/prova-707-aquario.mjs`, nos dois temas,
o trilho posto no meio da vida de uma tarefa já terminada mostrou a bolha dela
(«5 bolhas, 2 do log» e «5 bolhas, 3 do log»), e o retrato ficou em 0 pedidos
durante a reprise.

O que se perde sem o `retrato`/`nasceu`: a cor do **meio** da vida. A bolha da
reprise tem a cor com que a tarefa **terminou** (a que o log grava). E tarefa
que viveu menos de 1 s nunca entra (`VIVEU_NO_AQUARIO_MS`), como no ao vivo.

## A regra

Antes de abrir um escritor novo para um quadro-chave, pergunte se os eventos
que já se gravam carregam início e fim — intervalo reconstrói estado.

## Como está guardado hoje

A `faixa` sai na leitura (`a_consulta_diz_a_faixa_pela_op_e_o_arquivo_nao_a_guarda`),
para a bolha da reprise nadar na faixa do ao vivo. A cor do meio fica como
lacuna até as linhas `nasceu`/`retrato` existirem.
