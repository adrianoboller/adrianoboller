# A resolução do campo é premissa da linha de base

**Estado:** PENDENTE

## O que aconteceu

Na A0 da 0.21 (unificar a base do 707 com a do 495/496), os dois desenhos decidiam a mesma
estatística de jeitos diferentes — histograma `2 × p95` (aquário) e Welford sobre `ln(µs)` (IA) — e
os dois supunham, sem medir, que a duração do pedido serve de grandeza. O único lugar onde a duração
já existe é o `Acesso.duracao_ms` (`crates/phxsql-server/src/acesso.rs`), em **ms inteiros**.

## O que eu concluí primeiro, e estava errado

Que a disputa era histograma × média/desvio, e que repetir as duas regras sobre os `acessos.log`
reais decidiria. Repetidas, elas **empataram** (1/9.317 cada, com piso e exclusões), e o ruído que
sobrava sem o piso — 29 `commit` de 2–3 ms — não era anomalia nenhuma: era o degrau de 1 ms do campo.

## O que a medição disse

No log da sonda ODBC, **170 de 184** linhas têm `ms = 0`, e **152** delas são `sql`: em ms, a base
por digital do `sql` — a razão de ser do 495 — fica cega. E o `z` do medidor da IA devolve 0 quando o
desvio é 0, então uma série constante de 1 ms seguida de um pedido de 10 s não alarmaria. Os scripts
estão no Apêndice B do `docs/propostas/aquario-707.md`.

## A regra

Antes de escolher a estatística, meça a resolução do campo que a alimenta — e o que a fórmula faz
quando o desvio é zero.

## Como está guardado hoje

Só no desenho: o `Acesso` ganha `us` (§11.1) e o desvio ganha chão de 0,1 em `ln`, com o caso
«20 × 1 ms exatos + 10 s» como RED da A4. Até a A4 existir, nenhum teste segura isso.
