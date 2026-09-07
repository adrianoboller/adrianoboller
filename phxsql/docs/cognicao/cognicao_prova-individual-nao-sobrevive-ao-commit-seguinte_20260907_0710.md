# Prova individual não sobrevive ao commit seguinte

*Descoberto em 07/09/2026, 07h10, na corrida completa das 99 guardas.*

## 1. O que aconteceu

A guarda `fts-abrir-recusa-a-tabela` nasceu, foi provada **1/1** com o defeito
reposto, e eu segui trabalhando. Horas depois, no mesmo dia, a correção da
pista de leitura acrescentou um braço — `Err(_) if !escrever` — **dentro do
mesmo `match`** que a guarda cita como `trecho`.

A guarda passou a ser **QUEBRADA**: o executor não acha mais o trecho, então
não consegue repor defeito nenhum. E ela continuava contada no total.

Nada disso apareceu em portão nenhum: `fmt`, `clippy` e os 1.659 testes ficaram
verdes o tempo todo. **Quem acusou foi a corrida completa do catálogo**, no fim
da rodada — a mesma corrida que, na mesma passada, achou duas guardas do
Profiler quebradas desde uma rodada anterior.

## 2. O que eu concluí primeiro, e estava errado

Que provar uma guarda ao criá-la a deixava provada. A saída
`1 guardas: 1 provadas, 0 quebradas` é um retrato do **instante**, e eu a li
como um estado.

E concluí uma segunda coisa errada, mais fina: que a corrida completa era
**redundante** depois de eu ter provado cada guarda nova uma a uma. É o
contrário — as individuais provam que a guarda *nasceu* certa; só a completa
prova que ela *continua* certa depois de tudo o que veio depois.

## 3. O que a medição disse

Na corrida completa desta rodada, com 99 entradas:

| veredito | quantas |
|---|---:|
| provadas | 94 |
| redundantes | 4 |
| **quebradas** | **1** |
| não pegaram / estragaram | 0 |

E na corrida completa **anterior**, com 96 entradas, foram **2 quebradas** — as
duas do Profiler, envelhecidas desde a rodada do pedido 195, quando
`analisar_pedido` passou a devolver uma tupla. Nenhuma das três foi achada por
leitura; as três foram achadas pela corrida.

**Três guardas quebradas em duas corridas, e uma delas quebrou no mesmo dia em
que nasceu.**

## 4. A regra

**A corrida completa do catálogo roda no FIM da rodada, e não só quando uma
guarda entra.** Guarda quebrada continua sendo contada no total, então ela não
falha — ela infla.

## 5. Como está guardado hoje

Não há guarda para isto, e não pode haver: o executor **é** o instrumento, e um
conferidor que o julgasse mediria com a mesma régua. O que existe é o veredito
`QUEBRADA` — que já estava implementado e cumpriu o papel dele nas três vezes —
e o `docs/TESTES.md`, cuja tabela sai do `--json` de uma corrida e não de uma
lista digitada: **sem corrida não há tabela**, que é o desenho certo.

O que fica escrito é a **ordem**: a corrida completa é a última coisa da
rodada, depois do último commit de código. Rodá-la no meio dá um retrato que o
próprio trabalho seguinte invalida — foi exatamente o que aconteceu aqui.

E a entrada do catálogo carrega a própria história num comentário, porque
`catalogo.py` é auditável por gente: quem a ler daqui a seis meses vê que ela
já quebrou uma vez e por quê.
