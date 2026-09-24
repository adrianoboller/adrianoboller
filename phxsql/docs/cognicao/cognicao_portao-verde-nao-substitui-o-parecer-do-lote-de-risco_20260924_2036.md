# Portão verde não substitui o parecer do DBA num lote de risco
**Estado:** INFRUTÍFERO
**Causa:** o integrador comitou o lote integridade 2 (`18f1575`) com os quatro portões verdes na árvore exata e a revisão do DBA ainda correndo. O parecer chegou 35 minutos depois e bloqueou o 540, medido: pela sincronia do DbLink o índice único da mãe se corrompia em 5 de 5 rodadas (0 de 5 na base). Nenhum portão podia ver isso: a suíte não tinha teste que inserisse e cascateasse no mesmo punho, que era justamente o terceiro chamador que a frente não olhou.
**Prevenção:** lote que toca formato em disco, concorrência ou garantia de dado só vai ao commit depois do parecer do papel que o revisa; a pressão para comitar se atende comitando o que não depende dele (registro do juiz, MODELOS, pareceres), por caminho. Se o parecer bloquear depois de um commit, o pedido volta a parcial no commit seguinte, com o número, em vez de continuar dizendo feito.

## O que aconteceu

Às 20:05 os portões de integridade 2 (537, 538, 539, 540, 559) deram VERDE
na árvore exata, e o lote foi comitado e enviado em `18f1575`, com a frase
«a revisão do DBA ainda corre; o que ela achar reabre o pedido». Às 20:35 o
parecer (`docs/propostas/parecer-dba-integridade-2-2026-09-24.md`) liberou
quatro pedidos e bloqueou o 540: o `alterar_solto` abre um segundo punho da
mãe enquanto o punho da sincronia ainda tem páginas sujas, e o `Drop` do
punho velho regrava o índice velho por cima do reconstruído. O servidor
passou a aceitar código único repetido e filha órfã. O 540 voltou a parcial
em `08d1ce6`.

## O que eu concluí primeiro, e estava errado

Que «portões verdes na árvore exata» era o critério de commit, e o parecer
do DBA uma camada a mais que podia chegar depois — como tinha chegado LIBERA
nos lotes anteriores do dia. Os LIBERA anteriores eram a amostra errada: eles
mostravam que o parecer costumava concordar com os portões, não que os
portões viam o que o parecer vê.

## O que a medição disse

Base `a494f33`: 0 de 5 rodadas com o defeito. Árvore integrada (a mesma de
`18f1575`): 5 de 5. Portões na mesma árvore: fmt, clippy, suíte e catracas
verdes. A diferença entre as duas colunas é o que só o revisor mediu.

## A regra

Lote de risco espera o parecer; o que não depende dele pode ir antes.

## Como está guardado hoje

Não há guarda automática: é disciplina do integrador (papel A), escrita
aqui. A frente 533+542, integrada logo depois, ficou sem commit até os
pareceres do DBA (533) e do SEC (542) chegarem.
