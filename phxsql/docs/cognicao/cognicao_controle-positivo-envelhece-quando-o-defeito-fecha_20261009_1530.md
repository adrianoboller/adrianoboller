# O controle positivo de uma sonda envelhece no dia em que o defeito que ele usava é consertado

**Estado:** PENDENTE

## O que aconteceu

O `crates/phxsql-server/examples/sete-representacoes.rs` (pedido 669) usava
como controle positivo o `.ndx` da própria tabela cifrada: «tem de sair claro,
senão a sonda não acha nada e sai 2». Era o vazamento conhecido da §11.3. O
pedido 339 (achado 2) selou a página desse `.ndx` em 09/10/2026, e o exemplo
passou a sair 2 — não porque a sonda quebrou, mas porque o caso conhecido
deixou de existir.

## O que eu concluí primeiro, e estava errado

Que bastava inverter a expectativa do controle («`.ndx` tem de sair
cifrado»). Isso apaga o controle positivo: sobra só o negativo (`.reg`
cifrado), e uma sonda que não acha NADA passaria em todas as linhas — o
mesmo verde por engano que o controle existia para impedir.

## O que a medição disse

Com o controle passado para uma **tabela gêmea com o cofre desligado** (mesma
carga), a sonda acha o nome no `.ndx` e no `.reg` da gêmea («claro», os dois),
não acha no `.reg` da cifrada, e só então julga: `0 .ndx cifrado`,
`4 backup cifrado (.ndx copiado)`. Antes do conserto, na base `936fb3bd`, as
mesmas duas linhas diziam «claro».

## A regra

Controle positivo não se apoia num defeito: apoia-se numa gêmea que vaza por
construção (cofre desligado), porque o defeito um dia fecha e leva o controle
junto.

## Como está guardado hoje

No próprio exemplo, que sai 2 se a gêmea não vazar. Não há teste que rode o
exemplo na suíte; quem o roda é a mão (`cargo run --example
sete-representacoes -p phxsql-server`).
