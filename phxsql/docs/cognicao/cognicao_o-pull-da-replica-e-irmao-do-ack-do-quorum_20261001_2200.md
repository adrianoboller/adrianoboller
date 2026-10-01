# O pull da réplica é irmão do ack do quórum — e ele também não pode pedir a trava

**Estado:** PENDENTE

**Descoberto em 01/10/2026, 22:00 UTC**, desenhando o laço da réplica do pedido
207 (a escrita com quórum), antes de qualquer teste rodar.

## 1. O que aconteceu

O contrato do 207 (`docs/propostas/207-e-513p2-contrato-01-10-2026.md`, D-trava)
proibia uma coisa: o **caminho do ack** (`replicar_aguardar`) pedir a trava de
dados do master, porque o commit espera com a trava exclusiva na mão. A
implementação cumpriu — o cubo do quórum (`quorum.rs`) não conhece a trava, e a
guarda `quorum-ack-pede-a-trava` repõe o defeito.

Mas a réplica tem **dois** caminhos até o master, e os dois chamam o mesmo
servidor na mesma ordem: o canal do quórum e o **pull** de sempre
(`rodada_classificada` → `replicar`). O `replicar` toma `travar_dados()` na
primeira linha do corpo. Uma réplica que, no síncrono, fizesse um pull de
segurança a cada pulso cairia exatamente no abraço que o D-trava descreve: o
pull dela espera o commit que espera por ela, e o commit degrada pelo prazo.

## 2. O que eu concluí primeiro, e estava errado

Que o pull periódico ficava como **rede de segurança**: o laço da réplica
continuaria puxando a cada pulso, e o canal do quórum só aceleraria a entrega.
Parecia a escolha conservadora — «nunca tirar o caminho que já funciona».
Ela teria posto um commit inteiro no prazo (10 s, servidor parado) a cada vez
que o pulso da réplica caísse dentro da espera de um commit.

## 3. O que a medição disse

Desenhado do jeito certo (a réplica **não** puxa enquanto o master está
síncrono; só volta ao pull quando fica para trás, ou quando o master está
degradado e portanto não segura a trava):
`bancada/quorum/resultados-quorum-real.json` — 200 commits com 1 de 2 sem
nenhuma degradação, mediana 4,010 ms, faixa 3,438–9,152 ms.

O que sobrou do abraço, e está escrito como preço: **ligar o quórum a quente**
com a réplica no meio de um pull (o master acabou de responder «desligado» e
ela voltou a puxar) pode degradar o primeiro commit. A bancada espera três
pulsos depois de ligar, e o MANUAL manda o mesmo.

## 4. A regra

Quando uma guarda proíbe um caminho de pedir a trava, procure **todo** caminho
do mesmo cliente até o mesmo servidor — o irmão é quem chama as mesmas funções
na mesma ordem, e aqui o pull e o ack são o mesmo processo falando com o mesmo
master.

## 5. Como está guardado hoje

- `crates/phxsql-server/src/servidor.rs::esperar_pelo_master` — o comentário
  diz por que o pull não roda no síncrono;
- a guarda `quorum-ack-pede-a-trava` cobre o ack; **o pull não tem guarda
  própria** — o defeito «puxar a cada pulso no síncrono» não foi reposto e
  medido. É o buraco desta cognição, e é o que a impede de virar FRUTÍFERO.
