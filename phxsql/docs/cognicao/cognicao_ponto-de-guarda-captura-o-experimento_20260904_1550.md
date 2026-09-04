# O ponto de guarda registrou um experimento que o autor nunca ia commitar

*04/09/2026, 15:50 — hora da descoberta, não do commit.*

## 1. O que aconteceu

Com uma frente de conserto trabalhando na árvore, o gancho de parada cobrava
árvore limpa a cada resposta minha. Inventei o **ponto de guarda**: commitar o
trabalho em curso da frente com um rótulo dizendo «não conferido, não medido,
não é o conserto», para o trabalho não sumir com o contêiner efêmero.

Fiz cinco. O terceiro, `e88d065`, capturou o `Table::sincronizar` no instante
em que ele estava com a **ablação de medição** — sincronizando só `.log`,
`.ndx` e `.reg`, e deixando de fora `.trash`, `.bin`, `.memo` e `.reason`.

O comentário que eu commitei dizia, com todas as letras:

```rust
// ABLACAO DE MEDICAO -- nao vai para o repositorio.
```

E foi para o repositório, e para o `origin`. Quem fizer *cherry-pick* daquele
commit leva um `sincronizar` que perde dado.

Quem achou não fui eu: foi a **própria frente**, no relatório final, revisando
o que o ambiente tinha commitado por cima do trabalho dela.

## 2. O que eu concluí primeiro, e estava errado

Concluí que o rótulo bastava. O commit dizia «não conferido, não medido, não é
o conserto» — e eu tratei isso como se cobrisse todo o risco. Cobre **um**
risco: alguém ler meia entrega como inteira. **Não cobre o outro**, e o outro é
pior: que o estado capturado seja um estado que o autor **deliberadamente
escreveu errado** e nunca ia commitar.

E eu tinha um portão, e ele não servia para isto: eu conferia que **compila**.
Uma ablação de medição compila por construção — ela existe para rodar. «Compila»
não distingue conserto de experimento; distingue código de rascunho.

O segundo erro é mais fino e é o que ensina: eu rodei `git status`, li os dois
arquivos que apareceram (`volume.rs` e a catraca), julguei em cima daquilo, e
**depois** rodei `git add -A crates/phxsql-store`. Entre uma coisa e outra a
frente mexeu no `table.rs`. **O estado que eu julguei não é o estado que eu
commitei** — e o `git add -A` não me disse isso, porque ele obedece, não avisa.

## 3. O que a medição disse

| | |
|---|---|
| pontos de guarda feitos | 5 |
| que capturaram estado experimental | **1** (`e88d065`) |
| arquivos que eu inspecionei antes do `add` | 2 |
| arquivos que o `add -A` levou | **3** |
| o portão que eu apliquei | `cargo build` — passou |
| o portão que teria pegado | nenhum dos que eu tinha |

A árvore final está correta (os oito componentes de volta), e a catraca
`TETO_FSYNC_POR_FECHO_V2 = 8` **reprova** aquele estado — 4 `fsync` contra teto
8, e o lado «catraca frouxa» (`medido == teto`) é o que o pega. Mas a catraca
nasceu **depois** do commit: no instante do estrago não havia guarda nenhuma.

## 4. A regra

**Ponto de guarda de trabalho alheio em curso REGISTRA, não atesta — e o que
ele captura pode ser um experimento que o autor nunca ia commitar. Commite os
arquivos que você LEU, nomeados, nunca `-A`; e leia-os no mesmo instante em que
os adiciona, porque a árvore de uma frente viva se move entre o `status` e o
`add`.**

E o corolário sobre o portão: **«compila» não separa conserto de experimento.**
Um experimento compila de propósito. O que separa é ler o diff — ou uma guarda
que meça o comportamento, e essa é justamente a que ainda não existe quando o
conserto está sendo escrito.

## 5. Como está guardado hoje

- O `e88d065` **fica no histórico**, e fica de propósito: ele é o registro
  verdadeiro do que aconteceu, e reescrever o histórico para esconder um erro
  meu seria trocar a prova pela aparência. O que ele não pode é ser silencioso.
- Ele está nomeado **aqui**, no commit de integração, e na §8 do
  `docs/FORMATO.md`.
- A proteção que existe hoje e não existia então: `TETO_FSYNC_POR_FECHO_V2`
  reprova quem repuser aquele estado, e a guarda
  `fecho-da-janela-sincroniza-o-reg` reprova a falta do `fsync` no `.reg`.

**Onde o buraco ficou:** não há guarda que impeça *o próximo ponto de guarda* de
capturar outro experimento. A proteção é de processo — commitar arquivo lido e
nomeado — e processo não tem catraca. Se isto se repetir, aí sim vale um
conferidor que recuse commit contendo a marca «não vai para o repositório».
