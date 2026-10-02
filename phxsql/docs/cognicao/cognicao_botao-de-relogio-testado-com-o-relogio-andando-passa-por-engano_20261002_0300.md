# Botão que dispara um pedido, testado com o relógio andando, passa por engano

**Estado:** PENDENTE

## O que aconteceu

Pedido 190, `testes-web/casos/36-botoes-da-telemetria.mjs`. O «Atualizar
agora» da telemetria chama `volta()`, a mesma função que o relógio de 2 s
chama sozinho. A primeira versão do caso clicava o botão e esperava um pedido
`telemetria` em 900 ms. Na prova real (`prova-real-botoes.mjs`), com o botão
trocado por `() => {}`, o caso **passou**: a volta do relógio caiu na janela e
valeu pelo botão.

Na mesma frente, o clique em «Cadastro completo…» largava a pessoa numa tela em
branco: o rascunho que o cartão monta não trazia `indices_texto`, e
`desenharNovaTabela` estourava em `r.indices_texto.map`. E o caso reprovou duas
vezes só na bateria inteira, nunca isolado, porque o «Criar» do cartão termina
com `await montarArvore(); telaDiagramaER(db)` e o diagrama pintava por cima da
tela cheia aberta no passo seguinte.

## O que eu concluí primeiro, e estava errado

Que `waitForRequest` depois do clique bastava como prova de que o botão pede.
Não basta quando outra coisa também pede: o pedido que chega é do relógio, e o
teste verde diz «o botão funciona» sobre um botão morto.

## O que a medição disse

Com o defeito reposto (`$("#tlmAgora").onclick = () => {}`) a versão antiga
passou; a versão nova — pausa primeiro, depois o clique, com 1,5 s de espera —
reprova com `waitForRequest: Timeout 1500ms exceeded`. O teste do «Retomar» não
se isola do relógio pela mesma razão, e o caso diz isso no comentário em vez de
afirmar «na hora». No cartão: sem `indices_texto` o caso reprova com
`PAGEERR Cannot read properties of undefined (reading 'map')`; a corrida
inteira passou de 69/71 para 71/71 depois de esperar o diagrama ter a tabela
(`ER.esquemas`) em vez do botão que já estava na tela.

## A regra

Antes de afirmar que um botão dispara um pedido, **pare tudo o mais que também
dispara esse pedido** (pause o relógio) e só então clique; e depois de uma
ação que termina com um repintar assíncrono, espere o **efeito** desse repintar
(o dado novo no estado), nunca o seletor que já estava na tela.

## Como está guardado hoje

`testes-web/prova-real-botoes.mjs` repõe 13 defeitos num proxy reverso (sem
recompilar) e exige que cada um seja pego. O que **não** está guardado: o
«Retomar» pedir na hora segue sem prova isolada, e a pintura tardia do pedido
170 continua sem a marca de geração que a resolveria.
