# «Já aplicada» na réplica deixa o diário um evento atrás

**Estado:** FRUTÍFERO

**Evidência:** `crates/phxsql-server/tests/venda-inteira-na-queda-da-replica.rs::o_sigkill_entre_o_reg_e_o_diario_nao_duplica_a_linha`; `9e067e1b`

**Validação (08/10/2026):** guarda `replica-reaplica-inclusao-sem-olhar-o-reg` com veredito PROVADA em `bancada/guardas/ultima-corrida.json` (08/10/2026 06:25). O buraco da exclusão, nomeado na §5, não impede a promoção: a regra aqui vale para a inclusão. Promovido em 08/10/2026 pelo papel H.

## O que aconteceu

Pedido 699: a recuperação do grupo da réplica (`phxsql-store/src/marca.rs`,
`aplicar_evento_da_marca`) conferia só o diário. A inclusão grava o slot do
`.reg` **antes** do evento, e o `SIGKILL` entre os dois (gancho
`PHXSQL_TESTE_PARAR_NO_REG`, tabelas sem índice) reabria a réplica com o item
duplicado e o resto do grupo de fora: retrato **(0, 4, 0)** contra (1, 5, 1).

## O que eu concluí primeiro, e estava errado

O pedido e o contrato diziam: «inclusão com `rowid <= slots` e linha presente
**conta como já aplicada** sem chamar `inserir`» — a resposta da marca do
`COMMIT`. Na marca da réplica isso conserta o `.reg` e quebra o diário: o
evento continua faltando, a posição local (`eventos()`) fica uma atrás da
origem, a rodada seguinte pede o mesmo evento, e o `aplicar_evento` grava a
linha num slot novo e recusa a divergência **depois** de gravar — a duplicata
que se queria evitar, adiada uma rodada.

## O que a medição disse

Com o evento completado do payload do disco
(`Table::completar_o_diario_da_inclusao`), a reabertura dá (1, 5, 1) e a rodada
seguinte traz a segunda venda inteira — (2, 10, 2) —, o que só acontece se o
diário completado continua o da origem. O RED sem a guarda é o (0, 4, 0).

## A regra

Na réplica, o diário **é** a posição: toda idempotência que pula a gravação tem
de completar o evento que falta, nunca só contar.

## Como está guardado hoje

`venda-inteira-na-queda-da-replica.rs::o_sigkill_entre_o_reg_e_o_diario_nao_duplica_a_linha`
(passo 4 prova a continuidade) e a guarda
`replica-reaplica-inclusao-sem-olhar-o-reg`. O buraco nomeado: a **exclusão**
com a mesma queda (slot já livre, evento fora do diário) continua recusando,
porque completar o evento pediria a imagem do antes, que o slot livre não tem.
