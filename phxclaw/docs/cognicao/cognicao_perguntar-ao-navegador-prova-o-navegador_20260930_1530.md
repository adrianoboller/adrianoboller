# Perguntar ao navegador se o campo é inválido prova o navegador, não o nosso CSS

**Estado:** FRUTÍFERO

**Evidência:** `crates/phxclaw-ui-ir/tests/react.rs` — sem a regra `:user-invalid` no CSS o
teste falha na linha da cor da borda; com ela, passa (medido em 30/09).

## O que aconteceu

A data 31/02 era recusada pela validação, mas o print mostrava o campo igual a um campo
válido. Acrescentei `:user-invalid{border-color:…}` e escrevi a prova com
`el.matches(':user-invalid')`. Passou.

## O que eu concluí primeiro, e estava errado

Que o `matches` provava o destaque. Ele só diz que o **navegador** classifica o campo como
inválido — isso é verdade com ou sem a nossa regra de CSS. A prova passaria com o conserto
apagado.

## O que a medição disse

Trocando para `getComputedStyle(...).borderTopColor` do campo inválido contra um válido,
o teste passou a falhar sem a regra e passar com ela.

## A regra

Prova de interface confere o **efeito que o nosso código produz** (estilo calculado, texto
na tela), não o estado que o navegador já teria sem nós. É o alcance, na tela, de «teste
que passa por engano».

## Como está guardado hoje

A asserção da cor da borda no `react.rs`; o CSS é o mesmo do renderizador HTML.
