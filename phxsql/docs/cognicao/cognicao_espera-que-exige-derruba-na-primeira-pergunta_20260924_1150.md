# A espera que usa `exigir` derruba o teste na primeira pergunta

## 1. O que aconteceu

`com_o_source_continuo_a_replica_segue_sem_recusa`
(`crates/phxsql-server/tests/continuidade-da-replica.rs`) caiu em 2 de 7
corridas da suíte inteira em 24/09/2026. A mensagem foi sempre a mesma:
`"op":"posicao","database":"loja" -> ... database loja nao existe neste servidor`.

## 2. O que eu concluí primeiro, e estava errado

Que a falha era da rodada que estava entrando (PhxZip, `http.rs`), ou
«instabilidade» sem causa. Nenhum dos dois. O teste passava 15 de 15 isolado
e 12 de 12 com a CPU carregada. «Flake» não é causa, e a mensagem já dizia
onde estava o erro.

## 3. O que a medição disse

`esperar_eventos` pergunta à réplica recém-ligada quantos eventos ela tem, e
pergunta por `exigir`, que faz `assert!` no `ok`. Antes da primeira puxada, a
réplica ainda não tem o database, e o `posicao` responde «não existe». Esse é
justamente o «ainda não chegou» que a espera existe para esperar, mas o
`exigir` o trata como falha e derruba o teste de imediato, sem usar os 20 s de
prazo.

Não reproduzi a falha à mão (0 de 27). O conserto se sustenta pela leitura da
mensagem contra o código, e não afrouxa a exigência: continua sendo preciso
chegar ao número dentro do mesmo prazo.

## 4. A regra

Laço de espera não pode usar o ajudante que falha no primeiro erro. O estado
«ainda não existe» é resposta válida dentro de uma espera, e só vira falha
quando o prazo acaba.

## 5. Como está guardado hoje

`eventos_se_ja_houver` trata o `ok:false` como «ainda não» (-1), e só a
espera o usa. As afirmações diretas (`eventos_de_clientes`) continuam
exigindo `ok`. A suíte inteira passou (2.934) depois do conserto.
