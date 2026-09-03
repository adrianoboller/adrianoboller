# Frente travada aparece como «rodando» — o estado não mede nada, o relógio mede

## 1. O que aconteceu

A frente **I** (versionador: pacote e o número do Dockerfile) foi aberta às
16:38. Ao fechar a rodada, às 19:12, o `ListAgents` ainda a listava assim:

```
a25e871c612769725  ·  general-purpose  ·  running  ·  started 2h ago
```

«running», por duas horas e meia. Nesse intervalo eu perguntei duas vezes se a
resposta dela viria e deixei a integração à espera dela. O trabalho que ela
devia entregar — remedir o cliente musl e corrigir o número do Dockerfile —
tinha sido assumido por mim e commitado às 17:54, em `18ceb0e`, uma hora e
vinte antes.

## 2. O que eu concluí primeiro, e estava errado

Concluí que ela estava **lenta**, não parada: o último rastro que eu tinha
visto dela era um build `musl` do workspace inteiro, e build de workspace
inteiro em `musl` demora mesmo. Diagnóstico plausível — e errado por dois
motivos ao mesmo tempo.

O primeiro é que o build já tinha morrido (na cdylib do ODBC, que o
`empacotar.sh` documenta na bandeira `sem_odbc`) e ainda assim produzira os
binários. O segundo é que «lenta» e «parada» não se distinguem pelo estado que
o `ListAgents` mostra: ele diz `running` enquanto a tarefa não terminou nem foi
morta, e uma frente que parou de emitir eventos nunca termina sozinha. O estado
não estava mentindo — ele nunca respondeu a pergunta que eu estava fazendo.

## 3. O que a medição disse

Contando os eventos do próprio fluxo da frente (`tasks/<id>.output`, um JSON
por linha, cada um com `timestamp`):

| medida | valor |
|---|---|
| eventos emitidos | 86 |
| trabalho ativo | **5 min 15 s** (16:38:25 → 16:43:41) |
| maior lacuna entre dois eventos | **2 h 28 min 40 s** |
| eventos depois da lacuna | 1 — o meu `TaskStop`, às 19:12:21 |
| commits da frente | **0** |

Cinco minutos de trabalho e duas horas e meia de silêncio, com o mesmo rótulo
`running` nos dois. A última coisa que ela fez antes de calar foi um `TaskStop`
no próprio monitor seguido de `kill -9` no PID do build — o que não prova
causa, mas é o que está registrado.

E o número que decide: **2 h 28 min** de lacuna contra **5 min** de trabalho
ativo. Nenhuma frente desta base leva duas horas entre dois eventos quando está
viva; a maior lacuna sadia de uma frente ocupada é a de um `cargo test`, e o
`cargo test` do workspace inteiro aqui fecha em minutos, não em horas.

## 4. A regra

**Frente que não aparece no `git log` se afere pelo relógio do último evento
dela, nunca pelo estado que o listador mostra.** Lacuna maior que a operação
mais lenta que aquela frente poderia estar fazendo é frente morta: encerre e
assuma o trabalho, em vez de esperar uma resposta que não vem.

## 5. Como está guardado hoje

**Não está guardado — o buraco fica aqui nomeado.** Não há laço que vigie a
lacuna de evento das frentes abertas; a aferição foi feita à mão, com um script
de dez linhas lendo o `tasks/<id>.output`. Enquanto não houver esse laço, vale
o procedimento: ao fechar uma rodada, antes de esperar por qualquer frente,
medir a lacuna do último evento dela.

O que **está** guardado é a consequência de não ter feito isso: a regra da
cláusula pétrea de que *papel que não está cumprindo aparece como não
cumprindo* já valia para o zelador (que não roda de hora em hora) e para o
versionador (que esteve degradado pelo 403). Aqui ela ganha o alcance que
faltava — **um papel também deixa de cumprir por estar travado**, e travado é o
estado que mais se parece com trabalhando.
