# A conta que a origem faz para recusar tem de ser um TETO da conta que a réplica faz para partir

**Estado:** FRUTÍFERO

**Evidência:** `crates/phxsql-server/tests/transacao-acima-do-teto.rs::a_transacao_acima_do_teto_e_recusada_no_commit_e_a_que_cabe_chega_inteira`; `4b388e62`

**Validação (08/10/2026):** guardas `custo-da-transacao-sem-a-imagem` (PROVADA, 08/10/2026 03:24) e `commit-acima-do-teto-aceito` (PROVADA, 08/10/2026 16:19) em `bancada/guardas/ultima-corrida.json`. Promovido em 08/10/2026 pelo papel H.

## O que aconteceu

Pedido 685 (decisão do dono): a origem recusa no `COMMIT` a transação que a
réplica não aplicaria inteira. A réplica mede o que tem na mão
(`custo_na_transacao(imagem_do_fio)`); a origem tem de decidir **antes da
marca**, quando a imagem ainda não existe (os externos só vão ao `.memo` na
passada). Então a origem não pode fazer a mesma conta — faz uma estimativa.

## O que eu concluí primeiro, e estava errado

Que bastava a constante ser única: o mesmo `TETO_DA_TRANSACAO` dos dois lados
garantiria que «aceita na origem» implica «inteira na réplica». Não garante.
Com a mesma constante e uma conta da origem **menor** que a da réplica, a
transação aceita abaixo do teto chega partida — e o teste de «teto + 1
recusado» continua verde, porque ele só olha a recusa.

## O que a medição disse

Pelo soquete (`tests/transacao-acima-do-teto.rs`), 601 eventos de `Int8`:
S = 105.776 bytes. Com a conta da origem reduzida ao custo fixo do evento
(128 × 601 = 76.928) e o teto em S + 1 pela conta errada, a réplica recebeu
501 eventos (88.176 bytes), viu a transação incompleta acima do teto e partiu:
`transacoes_em_pedacos` = 1. Com a conta como teto (selo de todo externo,
«antes» de toda alteração), 0.

## A regra

Quando um lado recusa para que o outro nunca precise degradar, a recusa usa
um **limite superior** da medida do outro lado — e a prova tem de exercitar o
caso **aceito** perto do teto, com o outro lado medindo, não só o recusado.

## Como está guardado hoje

Guarda `custo-da-transacao-sem-a-imagem` (a conta só do custo fixo; cai a
prova do teto − 1) ao lado de `commit-acima-do-teto-aceito` (cai a do
teto + 1). As duas guardas deram PROVADA no provador (`bancada/guardas/ultima-corrida.json`, 08/10/2026: a primeira às 03:24, a segunda às 16:19). A frase anterior, «RED medido à mão; o provador não rodou nesta frente», envelheceu e foi trocada nesta data.
