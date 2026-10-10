# O corte do PITR por evento, que protegia o relógio torto, era o que partia a venda

**Estado:** PENDENTE

- **Quando:** 2026-10-10, 07:30
- **Onde:** `crates/phxsql-server/src/servidor/servico_backup_01.rs`
  (`reaplicar_diario_ate`), `crates/phxsql-server/src/replica.rs` (`Juntador`)
- **Pedido:** 299, fatia F3

## O que aconteceu

O PITR cortava o diário vivo **evento a evento** (`carimbo <= ate`), tabela a
tabela. O comentário acima da linha defendia a escolha com um número medido: o
carimbo não é monotônico (o bidirecional carimba com o relógio de outro
servidor, e um diário de três eventos tinha o do meio vinte e cinco anos
atrás), então «cortar a lista no primeiro que passou» jogaria fora os eventos
bons depois do torto. A prova vermelha da F0 mediu o outro lado: uma venda de
600 itens num `COMMIT`, com o `ate` no carimbo do primeiro item, voltava como
`(vendas, itens, pagamentos) = (1, 1, 0)` no restaurado.

E um segundo achado no mesmo caminho: a reaplicação inteira gravava sob a
unidade do diário da tomada da trava — **um id de transação para tudo** o que
o PITR reaplicou, o oposto do que a réplica faz.

## O que eu concluí primeiro, e estava errado

Que o furo era do **relógio**: bastaria trocar `carimbo <= ate` por «o maior
carimbo da transação <= ate» e manter o filtro. Transposto assim, o filtro
pula a transação que passou e aplica as seguintes que couberem — e a seguinte
que toca a mesma tabela para no rowid (a inclusão pulada não gerou o slot),
**depois** de ter entrado nas outras tabelas dela. A meia venda voltava pela
porta da guarda que o próprio comentário citava como proteção («pular uma
inclusão e aplicar a seguinte para na hora»): parar na hora, por tabela, é
justamente o que deixa as outras tabelas com a metade.

## O que a medição disse

- base `2e10a6b7`, corte por evento: `(1, 1, 0)`;
- PITR pelo `Juntador`, com a primeira transação fora do `ate` como
  **barreira** (o PostgreSQL: «só considera parar antes de um registro de
  COMMIT», `xlogrecovery.c`): `(0, 0, 0)`, `parou_na_transacao.motivo = relogio`;
- com a unidade por transação e o id adotado da original, o diário do
  restaurado sai com os ids do vivo (a prova compara as duas listas).

## A regra

Onde algo PARA no meio de um fluxo de várias tabelas, pare na **transação**, e
na ordem do **id**, nunca na do relógio: o relógio decide se a transação passa
do corte, o id decide onde ela mora na fila. Guarda que «para na hora» por
tabela é guarda que deixa a metade nas outras.

## Como está guardado hoje

Três guardas no catálogo: `299-pitr-corta-evento-a-evento`,
`299-pitr-rompida-so-para-a-tabela` e `299-pitr-uma-unidade-so`, com as provas
em `crates/phxsql-server/src/servidor/testes_pitr.rs`. O que não está
guardado: o evento sem id (volume anterior ao 676) que passa do `ate` para o
PITR inteiro — perde restauração, não entrega metade —, sem prova própria,
porque desde a virada forçada (R4) ele só existe em diário anterior a ela.
