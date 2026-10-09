# Tomada com `SKIP LOCKED` que não grava posse vale só até o commit

**Estado:** PENDENTE

**Evidência (para quem for validar):** a escolha da fila do modo fila (`docs/N8N.md` §13) leu as
três peças de tomada que a base já tinha. `phxclaw-event-bus::PostgresOutbox::claim_batch` faz
`SELECT ... FOR UPDATE SKIP LOCKED`, sobe `attempts`, marca `locked_at` e COMMITA — e o próprio
`WHERE` da tomada (`published_at IS NULL AND available_at <= now()`) não olha `locked_at`. Depois do
commit a linha está destravada e elegível: um segundo `claim_batch` leva a mesma mensagem. Não
medido contra o banco (é leitura do fonte); a prova do lado que ficou é
`crates/phxclaw-agent/tests/fila_workers.rs::dois_workers_nao_pegam_a_mesma_execucao`, cujo RED
(sem `FOR UPDATE SKIP LOCKED` na tomada do `task-graph`) levou a mesma execução duas vezes.

## O que aconteceu

O pedido era «ligue UMA das bibliotecas de fila que já existem, não escreva a terceira». As três
diziam `SKIP LOCKED` no fonte, e por isso pareciam equivalentes.

## O que eu concluí primeiro, e estava errado

Que `FOR UPDATE SKIP LOCKED` na tomada bastava para «dois workers nunca levam a mesma». Ele só
garante isso enquanto a transação da tomada está aberta. Uma execução de minutos não cabe numa
transação aberta; então a garantia tem de virar ESTADO gravado na linha (status `running` + um
prazo que o `WHERE` da próxima tomada respeita), e a cerca tem de ser um id do run (`active_run_uuid`)
que o batimento e o fim conferem. A outbox não grava esse estado — serve para publicar evento, não
para segurar trabalho.

## A regra

Para fila de trabalho longo, o `SKIP LOCKED` decide QUEM toma; a POSSE é uma coluna que a próxima
tomada lê (aqui o próprio `next_eligible_at` da linha `running`), e a cerca é um id que muda a cada
tomada. Peça que só tem o primeiro dos três não é fila de worker, por mais que o SQL pareça igual.

## Como está guardado hoje

`PostgresTaskJournal::claim_in` grava `status='running'`, `next_eligible_at = now() + prazo` e um
`active_run_uuid` novo; `renew` e `finish` só valem com o mesmo run. Três REDs medidos em
`fila_workers.rs`: sem o `SKIP LOCKED`, com a posse que não vence, e sem a cerca no fim.
