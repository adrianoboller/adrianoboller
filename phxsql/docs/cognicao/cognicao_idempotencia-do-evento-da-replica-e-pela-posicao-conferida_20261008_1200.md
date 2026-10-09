# A idempotência do evento da réplica é pela POSIÇÃO do diário, conferida — e a recusa tem de parar o resto

**Estado:** FRUTÍFERO

**Evidência:** `crates/phxsql-store/src/marca.rs::a_marca_do_grupo_nao_grava_por_cima_de_outra_historia`; `crates/phxsql-store/src/marca.rs::a_recuperacao_completa_o_grupo_da_replica_pela_posicao`; `550a1f2a`

**Validação (08/10/2026):** guarda `grupo-da-replica-sem-marca` com veredito PROVADA em `bancada/guardas/ultima-corrida.json` (08/10/2026 16:38). Promovido em 08/10/2026 pelo papel H.

## O que aconteceu

Pedido 682: a marca `.tx` passou a cobrir o grupo da réplica
(`gravar_marca_da_replica`, v5/v6), e a recuperação do arranque reaplica cada
evento pelo aplicador da réplica (`marca.rs::aplicar_evento_da_marca`).

## O que eu concluí primeiro, e estava errado

1. Que a idempotência da marca de `COMMIT` — **pelo rowid** («slot ocupado,
   passa adiante») — servia ao evento. Não serve: a alteração reaplicada grava
   a mesma linha e **acrescenta um evento** que a origem não tem, e o diário
   daqui deixa de continuar o de lá; a rodada seguinte romperia a tabela.
2. Que, trocada a regra para «aplica quando o diário tem exatamente `posicao`
   eventos», uma recusa por evento bastava. O teste
   `a_marca_do_grupo_nao_grava_por_cima_de_outra_historia` mediu o contrário:
   numa tabela de outra história com 1 evento, o 1.º da marca (posição 0)
   recusou — e o 2.º (posição 1) achou o comprimento «certo» e **gravou por
   cima** (`reaplicadas: 1`).

## O que a medição disse

- Comprimento sozinho não prova continuidade: a 1.ª recusa tem de parar o resto
  da marca. Com a parada, `reaplicadas: 0`, 2 impossíveis, 1 evento no diário.
- SIGKILL do `phxsqld` réplica no 3.º de 7 eventos do grupo, reabertura sem a
  origem: sem a marca, `(0, 3, 0)`; com ela, `(1, 5, 1)`, e a 2.ª venda chega
  depois (a continuidade não rompeu, porque o diário completado leva o carimbo
  e a origem de lá).

## A regra

Reaplicação de evento alheio é idempotente pela posição do diário **conferida
pelo conteúdo** (carimbo, operação, rowid), e a primeira recusa para o resto do
grupo — comprimento que bate não é prova de que a história é a mesma.

## Como está guardado hoje

`crates/phxsql-store/src/marca.rs::a_marca_do_grupo_nao_grava_por_cima_de_outra_historia`,
`crates/phxsql-store/src/marca.rs::a_recuperacao_completa_o_grupo_da_replica_pela_posicao`
e `crates/phxsql-server/tests/venda-inteira-na-queda-da-replica.rs` (guarda
`grupo-da-replica-sem-marca`, RED medido). **Atualizado em 08/10/2026:** o buraco que
esta seção nomeava (o bidirecional, `aplicar_grupo_bidi`, aplicava pela chave e
não gravava marca) foi fechado pelo commit `9e067e1b` («o bidirecional ganha a
mesma marca»). Não há buraco aberto nomeado aqui.
