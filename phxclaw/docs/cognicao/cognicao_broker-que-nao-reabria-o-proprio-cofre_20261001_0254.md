# Broker que não reabria o próprio cofre: todo segredo guardado travava o reinício

**Estado:** PENDENTE

**Evidência:** `crates/phxclaw-secret-broker/tests/reinicio.rs` falha com o leitor antigo
reposto (`broker_reabre_a_pasta_e_resolve_o_segredo_guardado_antes ... FAILED`) e passa com o
conserto; `tests/canal_telegram.rs` cai do mesmo jeito. Fica PENDENTE até o integrador
conferir no commit.

## O que aconteceu

Ao ligar o Telegram como canal do agente, o teste de reinício (processo novo, mesma pasta)
caiu em `JSON error: expected value at line 1 column 1` ao abrir o `SecretBroker`. O
`write_secret` grava `PHXSECRET1` + JSON; o `read_envelope` tira o prefixo; o `reload` — que
o `SecretBroker::new` chama — lia o arquivo inteiro como JSON. Resultado: **qualquer pasta
com um segredo guardado deixava de abrir**, e nenhum teste do broker reabria a pasta.

## O que eu concluí primeiro, e estava errado

Que o erro era do meu teste (pasta errada, ledger corrompido por dois processos). O ledger
e a chave estavam certos; o defeito era de dois leitores do mesmo formato, um deles
esquecido do prefixo.

## O que a medição disse

1 envelope de 601 bytes no cofre, segundo `SecretBroker::new` falhando sempre. Com um leitor
único (`parse_envelope`, usado por `reload` e `read_envelope`): reinício resolve o segredo.

## A regra

Formato em disco tem UM leitor. E guarda de persistência se prova reabrindo: teste que só
grava e lê no mesmo processo não alcança o `reload`.

## Como está guardado hoje

`tests/reinicio.rs` do broker e o reinício do `tests/canal_telegram.rs` do agente.
