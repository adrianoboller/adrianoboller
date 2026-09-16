# A vaga volta DEPOIS de o cliente ver o fim da conexão — 16/09/2026, 20:35

Pedido 267, papel F. O teste
`servidor::testes_das_threads::panico_dentro_do_atender_devolve_a_vaga_da_porta_de_dados`
caía 1 vez em 6 corridas da suíte `--lib` inteira sob carga alta.

## 1. O que aconteceu

O teste arma **três** pânicos dentro do `atender` com **teto de duas vagas**,
dispara três `ping` em fila e, no fim, cobra o total:

```rust
assert_eq!(s.panicos_de_teste.load(...), 0,
           "nem todos os panicos aconteceram -- a porta fechou antes");
```

Sob carga o log mostrava **dois** pânicos onde o cenário arma três. O `ping`
cuja conexão nem chega a ser atendida devolve `None` exatamente como o que
caiu no pânico — então o laço de cima passava e a conta de baixo reprovava,
sem dizer qual das duas coisas aconteceu.

## 2. O que eu concluí primeiro, e estava errado

Que o terceiro `ping` provavelmente **não chegava a abrir conexão** — thread
lenta, `accept` atrasado, algo do executor engolindo a conexão. Era plausível
e teria levado a mexer no laço de aceitação ou a dar mais prazo ao `ping`.

Errado nos dois pedaços. A conexão **sempre entrou** (`aceitas` subiu nas
mil rodadas medidas, `pings_nao_aceitos=0`), e o que faltou não foi prazo: foi
**vaga**. E o segundo engano foi de método, e custou meia hora: a primeira
tentativa de reproduzir rodou o teste sozinho **300 vezes com a máquina
carregada por fora** (load 10–12) e deu **0 falhas** — eu estava apertando a
máquina quando o vizinho que importa é o que disputa CPU **dentro do
processo**.

## 3. O que a medição disse

Instrumento: dois contadores só de teste dentro do laço de aceitação **de
produção** (`#[cfg(test)] aceitas_de_teste` e `sem_vaga_de_teste`, em
`servidor.rs`), mais a sonda `sonda_267_corrida_dos_tres_panicos`
(`#[ignore]`), rodada **dentro da suíte `--lib` inteira**
(`--include-ignored --test-threads=4`) com três suítes de carga ao lado
(load 13–15):

| forma do teste | rodadas | com pânico faltando | contadores nas que faltaram |
|---|---:|---:|---|
| antiga (três `ping` em fila) | 500 | **20 (4,0%)** | `aceitas=3 sem_vaga=1` nas **20** |
| nova (espera a vaga entre uma e outra) | 500 | **0** | `sem_vaga=0` |
| antiga, sonda SOZINHA com carga externa | 340 | **0** | — |

A janela que explica tudo, medida com espera ocupada (um `sleep` de 5 ms
mediria o próprio `sleep`) entre «o cliente viu o fim da conexão» e «a vaga
voltou»:

| p50 | p90 | p99 | máximo |
|---:|---:|---:|---:|
| 1–2 µs | 4,1–6,9 ms | 7,6–22,5 ms | **36,7 ms** |

O soquete morre no desenrolar do pânico (é o `fluxo`, dentro do quadro do
`atender`); a permissão RAII morre depois, quando a thread acaba. Com teto 2,
o terceiro `ping` que chega dentro das janelas dos dois anteriores leva recusa
— que é o comportamento **certo** acima do teto.

Prova real nos dois sentidos, com o defeito do motor reposto (`ManuallyDrop`
sobre a `Permissao`, guarda `permissao-de-dados-sem-raii`):

| corrida | defeito reposto | conserto |
|---|---|---|
| teste sozinho | **10 vermelhos em 10** (sempre na conexão 0) | — |
| provador de guardas | **PROVADA**, 1/1 caíram | árvore limpa verde |
| suíte `--lib` inteira sob carga (load 13,9–15,9) | — | **6 verdes em 6** |

## 4. A regra

**O fim da conexão que o cliente vê não é o fim da thread que a atendeu — e
teste que abre a conexão seguinte sem esperar o recurso voltar cobra do motor
uma garantia que ele não dá.**

E a irmã, de método: **para reproduzir corrida entre testes, monte o vizinho
dentro do processo; carga na máquina não é o mesmo ambiente** (0 em 340 fora,
20 em 500 dentro).

## 5. Como está guardado hoje

- O teste consertado está em `crates/phxsql-server/src/servidor.rs`, com a
  espera da vaga **dentro** do laço: cada conexão prova a sua — panicou e
  devolveu a vaga — e a próxima só parte com a vaga de volta. As três
  continuam entrando com teto de duas, que é o que prova o reaproveitamento.
- Os dois contadores (`aceitas_de_teste`, `sem_vaga_de_teste`) ficam no laço
  de produção sob `#[cfg(test)]` e entram na **mensagem** da falha: se algum
  dia ela voltar, diz se a conexão entrou ou não, em vez de confundir as duas
  causas como antes.
- A sonda fica na árvore, `#[ignore]` por custo, com a receita de medição no
  próprio comentário — o roteiro não morre com a sessão.
- Lição longa em `docs/TESTES.md` §19; pedido **267** fechado no
  `docs/PENDENCIAS.md`.
- **O buraco que fica:** o conserto é no teste, e não no motor — não há defeito
  no motor aqui. Mas a mesma forma («dispara N conexões em fila e cobra o
  total no fim») pode existir em outros testes de soquete desta casa, e isso
  **não foi varrido**. Quem varrer, procure teste que abre conexão nova sem
  esperar a anterior devolver o recurso.
