# Não esperar a trava não mata o ciclo: quem repete também espera

**Estado:** PENDENTE

## O que aconteceu

Pedido 516: o elo da cascata que só o COMMIT descobre passou a tomar a trava
da linha filha. O COMMIT está com a trava de dados na mão, então eu fiz a
tomada **sem espera** (`Espera::Nenhuma`): barrado, ele recusa com
`EM_TRANSACAO`, `repetir: true`, nada gravado. O papel C mediu o laço que isso
criou. T1 muda a mãe `a`, T2 muda a `b`, as filhas `c→a` e `d→b` nascem por
outra sessão, T1 altera `d` e T2 altera `c`. Os dois COMMITs se barraram por
**1.870 rodadas em 11 s**, com um prazo de 3 s, porque o COMMIT não confere o
prazo. Só uma terceira sessão desfez o laço, matando os dois.

## O que eu concluí primeiro, e estava errado

Que tentar uma vez e recusar era **imune** a impasse. O próprio `travas.rs`
diz: «sem espera não há grafo de espera, e sem grafo não há ciclo». Mas o
recado manda repetir, e o cliente repete: a espera saiu do servidor e foi
morar no cliente, e o grafo voltou com ela. Nenhum dos dois é o dono do
«esperar», então nenhum dos dois percebe o ciclo.

## O que a medição disse

- Sem desempate: 20 rodadas e os dois ainda em `EM_TRANSACAO`
  (`dois_commits_que_se_barram_cedem_pela_mais_nova` vermelho).
- Cada COMMIT barrado anota quem o barrou (`commit_barrado_por`). Se a corrente
  volta a ele, há ciclo, e a mais nova cede: `TRANSACAO_ABORTADA`,
  `repetir: false`, travas soltas na hora. Assim termina em ≤ 3 rodadas, nas
  duas ordens.
- Ceder **sem soltar as travas**, devolvendo a lista como a recusa comum, deixa
  a mais velha barrada. É o segundo vermelho medido.

## A regra

Toda recusa que manda repetir é uma espera; antes de trocar «esperar» por
«recusar e repetir», desenhe o grafo com a repetição dentro e dê a ele um
desempate.

## Como está guardado hoje

Guardas `ciclo-de-commits-sem-desempate` e `quem-cede-no-ciclo-segura-as-travas`.
**Onde o buraco ficou:** a espera comum, sem ciclo, continua sem teto, porque o
COMMIT não confere o prazo da transação. É pedido novo, P3 do parecer do papel C.
