# Teto em passos não é teto em tempo

- **Quando:** 2026-09-02, 21:05
- **Onde:** `phxsql-sql/src/rotina.rs` (`PASSOS_MAX`) e `docs/CONCORRENCIA.md`
- **Custo:** zero, porque a medição veio antes do conserto — teria sido um dia
  de trabalho no alvo errado

## O que aconteceu

O mapa da concorrência dizia, sobre as cinco seções que rodam gatilho `BEFORE`
com a trava global na mão:

> A duração dessas cinco **não tem teto**: é o que quem escreveu o gatilho
> quiser. Nenhum desenho de trava conserta isso.

Aceitei a frase e escolhi essas cinco como o primeiro alvo. Antes de mexer,
fui ao avaliador — e o teto estava lá, escrito, com o motivo certo no próprio
comentário: `PASSOS_MAX = 1_000_000`, *«num gatilho, roda com a trava de dados
na mão»*.

## O que eu concluí primeiro, e estava errado

Duas vezes, e a segunda é a interessante.

Primeiro: aceitei «não tem teto» sem conferir. Era afirmação sobre o código,
lida de um mapa do código — e mapa não é o território.

Depois, ao achar o `PASSOS_MAX`, quase escrevi «então o problema não existe».
Também errado: **um milhão de passos só é um teto de trava se um milhão de
passos for rápido**, e ninguém tinha convertido passos em milissegundos. Teto
que ninguém traduziu para tempo é número citado, e número citado é número que
não se mede.

## O que a medição disse

`cargo run --release --example custo-do-gatilho -p phxsql-sql`, mediana de 3:

| caso | tempo |
|---|---:|
| pior caso — `WHILE TRUE`, gastando o orçamento inteiro | **18,3 ms** |
| corpo comum — um `IF` e uma soma | **1 µs** |

E o `BEFORE` roda sobre `MotorNulo`: **não alcança o motor**, não lê nem grava
tabela. Não há I/O ali para esticar o número.

## A regra

**Orçamento em unidade interna não é garantia até alguém convertê-lo para a
unidade que dói.** Passos, iterações, nós visitados, bytes analisados — nada
disso limita uma trava; só o relógio limita. Quem escreve um teto em passos
deve o número em milissegundos junto, e quem lê um teto em passos não sabe o
que ele vale até medir.

## Como está guardado hoje

O medidor `custo-do-gatilho.rs` fica, e a frase falsa do `CONCORRENCIA.md` foi
riscada com o número no lugar.

**O que muda de plano:** as cinco seções do gatilho deixam de ser o primeiro
alvo. As 27 de leitura com varredura passam a ser as candidatas — **e isso é
hipótese, não conclusão**: elas ainda não foram medidas em tempo de trava, e
trocar um palpite por outro não é medir. Foi exatamente esse passo que esta
cognição existe para impedir.
