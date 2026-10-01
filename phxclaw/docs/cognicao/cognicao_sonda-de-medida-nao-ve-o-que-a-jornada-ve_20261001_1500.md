# A sonda de altura passou; a jornada real pegou o cartão por cima do botão

**Estado:** PENDENTE (a prova roda em `tests/desktop/pwa_ponte.mjs` e na sonda B2 do
`qualificacao/qualificar.mjs`; promove com o commit da SP000027)

## O que aconteceu

No lote 4 da qualificação, a grade virou cartões abaixo de 640 px. A sonda B2 media a altura
mediana das linhas e passou (185 → 75 px em Tarefas). O `pwa_ponte.mjs`, que faz a jornada do
usuário no celular (abrir tarefa, responder), caiu para 6/7: os cartões vazavam por cima do painel
de detalhe e tomavam o toque do RESPONDER.

## O que eu concluí primeiro, e estava errado

Que «as linhas cabem» provava «a grade é utilizável no celular». A medida certa respondia a
pergunta errada: altura é condição necessária, não suficiente.

## O que a medição disse

A sonda B2 passou a conferir também que o elemento no ponto do botão RESPONDER é o próprio botão
(`elementFromPoint`); com o defeito reposto (snapshot s10) ela fica vermelha.

## A regra

Sonda de medida não substitui jornada: toda checagem de layout que diz «utilizável» precisa de
pelo menos um toque real no controle que importa.
