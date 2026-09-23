# Cognição: o pedido carregava a decisão do dono E a revisão dela, e eu li a primeira

Descoberta em 23/09/2026, 04:55 UTC, quando a frente P devolveu o `PSCH` v10 e
nomeou — sem ser perguntada — que o meu próprio contrato citava a metade
superada do pedido 289.

## 1. O que aconteceu

Montei o contrato da frente P com esta frase: *«289 — uma coluna, 8 bytes,
`u64` de NANOSSEGUNDOS»*. A frente implementou **duas** colunas, 16 bytes —
`rowstamp`, contador puro, e `rowtime`, relógio comum — e escreveu no relatório:

> «A linha 313 do `PENDENCIAS.md` registra o dono **decidindo de novo**, às
> 07:5x de 17/09/2026, depois da régua dos motores. A árvore está certa e o
> briefing estava atrás. Não redecidi; só estou nomeando, porque **quase reverti
> duas colunas para uma**.»

Conferido no fonte, e a frase está lá por extenso: **«MUDA EM PARTE, e o dono
decidiu de novo: DUAS COLUNAS, 16 bytes.»** Com o motivo medido junto: os três
motores maduros convergem em que **o relógio empata de propósito** — o
PostgreSQL chama isso de *feature* por escrito — e em que a ordem mora num
**contador**: `xmin` no PostgreSQL, `DB_TRX_ID` no InnoDB.

## 2. O que eu concluí primeiro, e estava errado

Concluí que **tinha lido o pedido**. E tinha — li o corpo, achei a decisão do
dono, e parei ali. O que não me ocorreu é que **o corpo pode conter a decisão e
a revisão dela**, em ordem cronológica, e que a que vale é a **última**.

O agravante é que eu já tinha escrito a lei contra isso, **noventa minutos
antes**, na cognição das 03:30: *«Antes de pedir uma decisão, leia o CORPO do
pedido até o fim.»* Cumpri a letra e perdi o alcance — porque «até o fim» eu
entendi como «até achar a decisão», e não como «até o fim».

E o erro quase custou o trabalho da frente, não só a minha reputação: ela mediu
o defeito reposto e achou **964 de 1.000 linhas no mesmo milissegundo**, com o
pai empatando com a filha na primeira rodada. Se ela tivesse obedecido ao meu
contrato em vez de conferir a fonte, teria revertido duas colunas para uma — e
a pétrea «impossível o filho ter a MESMA data do pai» voltaria a ser falsa, com
o teste passando.

## 3. O que a medição disse

- O pedido 289 traz **duas** decisões do dono, do mesmo dia: 07:10 e 07:5x. A
  segunda começa com **«MUDA EM PARTE»** e a primeira não traz marca nenhuma de
  que seria revista.
- **8 bytes → 16 bytes**, e a diferença não é de tamanho: é de **natureza**.
  Uma coluna de relógio não ordena; um contador não data. O desenho de uma
  coluna só tentava que um campo fizesse as duas coisas.
- Defeito reposto pela frente, com o número no próprio teste: **964 de 1.000**
  carimbos empatados quando o carimbo vem do relógio. A ordem do dono não se
  cumpre com relógio, e agora isso está medido em vez de argumentado.
- Custo real do meu erro: **zero**, e só porque a frente foi à fonte em vez de
  obedecer ao contrato. Contrato de integrador não é fonte — é resumo, e resumo
  envelhece.

## 4. A regra

**Quando um pedido carrega decisão do dono, leia até o fim do corpo procurando
a REVISÃO dela — e cite a última, com a hora.** Decisão do dono não é evento
único: ele decide, mede, e decide de novo. Um corpo de pedido é um diário, não
um veredito.

E o corolário, que é o que faz a regra valer para quem monta contrato:
**contrato de frente cita a fonte com `arquivo:linha`, não de memória** — e a
frente tem a obrigação de conferir a citação antes de obedecer. Aqui foi a
frente que pegou, e é assim que tem de ser: o contrato é a hipótese, a fonte é o
dado.

## 5. Como está guardado hoje, e onde o buraco ficou

**Guardado:** o `docs/board-rodada-0.19.md` foi corrigido no mesmo passo (a
linha do 289 diz hoje «DUAS colunas, 16 bytes», com a hora 07:5x). O
`docs/FORMATO.md` documenta as duas colunas com o número medido. A frente
deixou 12 provas em `crates/phxsql-store/tests/carimbo-e-faixa.rs`, cada uma com
o vermelho medido.

**O buraco, e é o mesmo de antes com outra cara:** o `PENDENCIAS.md` continua
sem estado que distinga «espera o dono» de «decidido, espera engenharia» — e
agora se sabe que falta mais um eixo: **não há marca nenhuma na primeira decisão
dizendo que ela foi revista**. Quem varrer o corpo procurando «DECIDIDO PELO
DONO» acha as duas e não sabe qual vale; quem parar na primeira acha a errada.

**O que NÃO é o conserto:** casar «MUDA EM PARTE» por texto. É a mesma armadilha
já medida nesta casa com as oito interpolações de erro cru, das quais só duas
eram defeito — casar texto acha o caso e perde a família. O conserto é de
**estrutura**: a decisão que vale é a última, então ela precisa de lugar próprio
no pedido, não de posição no meio da prosa.
