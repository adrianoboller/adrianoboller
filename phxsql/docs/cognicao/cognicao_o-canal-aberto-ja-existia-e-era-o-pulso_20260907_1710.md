# O canal aberto já existia, e era o pulso — pedido 207

**07/09/2026, 17h10. Frente F2 (cluster).**

## 1. O que aconteceu

O pedido 207 (transação com quórum) foi decidido pelo dono «pela **rota do
canal aberto**», e a linha do `PENDENCIAS.md` apontava onde essa rota moraria:
`bidirecional.rs`. A tarefa desta frente mandava, antes de qualquer código,
**medir a premissa**: o canal aberto hoje leva o quê, em que direção, e com
que latência?

Abrir o `bidirecional.rs` respondeu na primeira linha do cabeçalho:

> *«A parte funda da replicação bidirecional (multi-master), **sem rede**.»*

Ele resolve conflito por carimbo, decide quem vence no empate, evita o laço
infinito pela origem gravada no evento e identifica a linha pela chave única.
**Não abre soquete, não conecta, não empurra.** Não há canal ali, e nunca
houve.

E o canal existe — noutro arquivo. O `laco_do_pulso`/`pulsar` do `servidor.rs`
abre uma conexão de **cada nó para cada outro nó**, autentica com a credencial
do cluster e a mantém viva num laço trocando `cluster_pulso`.

## 2. O que eu concluí primeiro, e estava errado

**«O master é pull, então ele não alcança ninguém — a rota do canal aberto
precisa ser construída.»** É o que a linha do pedido diz, com o motivo escrito
e correto: *o source abre uma porta de entrada e nunca precisa alcançar a
réplica de volta* — a propriedade de firewall que o desenho comprou.

Está certo **para a replicação** e **errado para o cluster**, e eu li os dois
como a mesma coisa porque o cluster usa a replicação por dentro. Num cluster o
pulso já obriga todo mundo a alcançar todo mundo: **a propriedade de firewall
já tinha sido gasta**, no dia em que o pulso nasceu, e não seria o quórum a
gastá-la. Eu ia projetar a conquista de um terreno que já era nosso.

O segundo engano, menor e mais caro de descobrir tarde: **contar canais pela
fórmula.** N×(N−1) daria 6 para três nós, e continuaria dando 6 depois de um
laço morrer. Contei na telemetria de cada nó — e aí apareceu que o
`pulso-supervisor` (a thread que eu mesmo tinha acabado de escrever) entrava na
conta e inflava o número em **um por nó**: 9 onde eram 6. Zelador contado como
canal é medidor que mente para mais.

## 3. O que a medição disse

`bancada/quorum/canal.py`, três `phxsqld` em `127.0.0.1`, 60 voltas, conexões
**quentes** (abertas e autenticadas uma vez, como o `pulsar` faz):

| o que | medido |
|---|---|
| canais abertos no cluster de três, contados na telemetria | **6** |
| canais que o **master** já segura para as réplicas | **2** |
| piso do canal — `cluster_pulso` sem dado nenhum | **0,089 ms** (0,069–0,232) |
| empurrar um evento pela conexão quente | **0,466 ms** (0,337–34,988) |
| quórum 2-de-3 pelo canal | **0,447 ms** |
| quórum 3-de-3 pelo canal | **0,498 ms** |

Dois resultados, e o segundo é o que eu não esperava:

1. **O canal já está lá**, e o master é dono de duas pontas dele.
2. **O canal é barato, e o caro é aplicar.** O piso — 0,089 ms — é ~5× menor
   que levar um evento. O preço de um quórum é o trabalho de **aplicar**, não o
   de **falar**. Eu teria escrito o contrário se não medisse.

E ele explica a medição anterior desta casa: a `bancada/quorum/medir.py`
publicou `2-de-3 = 0,661 ms` abrindo o caminho a cada volta; pelo canal quente
são **0,447 ms**. A diferença é o aperto de mão que aquela média carregava e
que o master **não pagaria** — ele já tem a conexão aberta.

## 4. A regra

> **Antes de projetar a rota que falta, conte as que já existem — e conte-as no
> registro que as mostra vivas, nunca pela fórmula que as previu.**

E o corolário sobre premissa herdada: **uma premissa verdadeira num subsistema
não é verdadeira no que o usa.** «O master é pull» é lei da replicação e
mentira do cluster, e a frase sobreviveu duas rodadas porque ninguém tinha
motivo para lê-la de novo.

## 5. Como está guardado hoje

- O medidor: `bancada/quorum/canal.py` → `bancada/quorum/resultados-canal.json`.
- O parecer, com o que falta e em que ordem:
  `docs/propostas/quorum-de-escrita.md`, registrado no `LEIA-ME.md` da pasta.
- A correção da premissa no documento da área: `docs/REPLICACAO.md` §19.7, e a
  linha nova na §2.4 do `docs/CLUSTER.md`.
- O que **não** entrou, de propósito: o commit por quórum. Faltam quatro peças,
  e a primeira é decidir o que o «ok» da réplica significa — *recebeu*,
  *aplicou*, ou *aplicou e sincronizou*. **Não se entrega meia transação**, e um
  quórum sem definição de garantia não é meia funcionalidade: é uma promessa
  errada inteira, do naipe do `QUORUM` do Cassandra® que quer dizer `mmap`.
- O que entrou: só o campo `cluster.quorum_minimo`, porque **mudança de formato
  entra cedo**, com o servidor declarando `"quorum_imposto": false` ao lado
  dele. Campo que finge efeito é pior que campo ausente — o
  `recursos.cache_paginas` passou três versões mentindo, e a diferença aqui é
  que o servidor **declara** que não lê.
