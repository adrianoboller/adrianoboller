# Resposta atrasada ressuscita o id de sessão que o giro matou

**Estado:** PENDENTE

## O que aconteceu

Pedido 784. A `testes-web/prova-707-aquario.mjs` caía em «faça login antes»
logo depois de abrir o aquário com carga (300 mil linhas, quatro conexões
agrupando). A `api()` da página fazia `if (j.sessao) est.sessao = j.sessao` em
TODA resposta. O servidor devolve no corpo o id que leu na chegada do pedido.
A sondagem do aquário (2 s) saiu com o id A e ficou presa atrás da trava de
dados; enquanto isso o `liberar_execucao` do `entrar()` girou A para B (767; o
login gira desde o 719) e A morreu. A sondagem voltou depois, trazendo A, e a
página adotou o morto: o pedido seguinte foi recusado.

## O que eu concluí primeiro, e estava errado

Que a prova lia errado: o pedido dizia «cai em faça login antes depois da
carga», e o primeiro palpite foi prazo de sessão vencido pela carga longa. Não
era o teste. É um defeito da tela, e alcança gente de verdade: qualquer pessoa
que libere a senha de execução enquanto o aquário ou a telemetria sondam com
a trava ocupada perde a sessão.

## O que a medição disse

`testes-web/casos/54-giro-com-pedido-em-voo.mjs` segura a resposta de um
`bancos` já atendido (id velho) até o `liberar` terminar: **2/2 vermelhos** no
binário da base `b48389b3` («sessão VOLTOU ao id velho, e o pedido seguinte foi
recusado: [SP000025]»), **2/2 verdes** com o conserto. A prova 707, depois,
deu três corridas inteiras verdes.

## A regra

Adote o id de sessão de uma resposta só quando o pedido saiu com o id que
ainda é o vigente; quem saiu com um id já trocado não tem autoridade para
trocá-lo de novo.

## Como está guardado hoje

Conserto em `crates/phxsql-server/ui/index.html` (`api()`, `enviada`), guarda
no caso 54 da bateria. O buraco: o servidor continua devolvendo o id lido na
chegada, e qualquer outro cliente HTTP que adote cegamente o `sessao` do corpo
cai no mesmo poço; nenhum outro cliente nosso faz isso hoje (conferido por
`grep` de `X-Sessao` em `ui/`).
