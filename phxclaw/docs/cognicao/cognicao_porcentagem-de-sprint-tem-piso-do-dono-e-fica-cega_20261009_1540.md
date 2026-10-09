# A «% que falta das sprints» tem piso do dono e fica cega ao trabalho dentro da sprint

**Estado:** PENDENTE

**Evidência (o que existe, e por que ainda não promove):** estudo
`docs/ciencia/estudo_previsao-do-que-falta_20261009_1526.md`, extrator
`docs/ciencia/extratores/serie_do_que_falta.py` + `modelos_do_que_falta.py` (duas corridas
idênticas). Medido: `falta%` = 13/39 = 33,3% em 5 rodadas seguidas (02/10 05:32 → 09/10 14:24)
enquanto a lacuna de absorção caiu de 54 para 18 itens; 3 sprints BLOQUEADA (dono) com 0 saídas em
18 sprint-rodadas. Fica PENDENTE até a conferência da previsão datada do §7 do estudo.

## O que aconteceu

Pedido: prever em quantas rodadas a `falta%` do dossiê chega a zero. A série por commit do
`SPRINTS.md`, lida pelo mesmo leitor do dossiê, tem 15 pontos; por rodada, 4 (dia) ou 6 (registro
em `docs/sprints/`). Tendência linear perde do ingênuo (MASE 1,31). Toda rodada que fechou sprint
também abriu. Com escopo como tem entrado, P(não chegar a zero em 50 rodadas) = 100%, e o motivo
dominante não é estatístico: 3/39 = 7,7% dependem do dono.

## O que eu concluí primeiro, e estava errado

Na série secundária (absorção), a simulação disse «a lacuna zera em 2 rodadas, IC90 [1; 3]».
Errado: o modelo deixava as rodadas fecharem os 7 itens que o próprio `SPRINTS.md` diz depender de
recurso do dono (iMessage, voz ao vivo, RAPL, nuvem), à taxa dos itens fáceis já fechados. Com o
piso tratado como bloqueado, zero passa a 100% de não chegar sem o dono; só «até o piso» sai em 2
rodadas — e mesmo esse é otimista, porque o que sobra é o difícil.

## A regra

Antes de prever «quando chega a zero», separe o estoque que nenhuma rodada de trabalho remove
(bloqueado por dono, recusado, decisão de produto) e conte as saídas dele no histórico; se forem
zero, o zero é decisão, não previsão. E métrica binária de item grande (sprint) não serve para
medir ritmo: use o grão em que o trabalho aparece (chaves, itens).

## Como está guardado hoje

Só no estudo e nos dois scripts. Nenhum gerador do dossiê usa ainda o piso nem o grão fino; a
próxima hipótese (L-002, chaves feitas por rodada) é o teste que pode promover ou matar a segunda
metade da regra.
