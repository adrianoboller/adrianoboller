# A régua que resolve por nome esconde a chamada qualificada — e um ajudante a mais tira seções dela pela profundidade

**Estado:** PENDENTE

## O que aconteceu

Pedido 627: a catraca `rede-ou-espera` do `bancada/concorrencia/mapa-da-trava.py`
dizia **0** com a espera de garantia do 605 (`nascendo::esperar`, um
`Condvar::wait`) acontecendo com a trava global na mão. A agulha de «espera» era
só `sleep`.

## O que eu concluí primeiro, e estava errado

Que bastava ensinar a agulha (`.wait(`, `.wait_timeout(`…). Ensinada, a catraca
**continuou em 0**. A espera aparecia no mapa, mas com confiança 0,5:
`crate::nascendo::esperar(` resolvia pelo nome nu `esperar`, que tem **quatro**
definições na árvore (o `asn1` que analisa bytes, o `email` que lê soquete, o
`saude_do_disco` e o `nascendo`) — 2 de 4 alcançam um `wait`, e a classe só conta
o que passa de 0,999.

E um segundo erro, já com a régua certa: ao dar prazo à espera, separei o laço
num ajudante (`esperar_ate`). O número caiu de **11 para 8** — sem o código
esperar menos. Os três caminhos das cargas de idiomas ficaram a seis saltos, e a
régua (`SALTOS = 5`) parou de alcançá-los.

## O que a medição disse

- agulha antiga: 0; agulha nova sem resolução qualificada: 0; as duas: **11**;
- as 11 são todas `nascendo::esperar` nos caminhos que criam sob a trava;
- com o ajudante extra: 8; com o laço dentro do próprio `esperar`: 11 de novo;
- a mesma espera dentro do `Table::abrir_com` continua abaixo do corte (chega
  por `abrir`, com 23+ homônimos): o 11 é piso, não total;
- de brinde, a régua passou a ver o `esperar_trava` da transação dentro do
  `empilhar` (classe `codigo-do-dono`, que tem precedência, então não entra no 11).

## A regra

Quando a régua resolve por nome, `modulo::funcao(` tem de resolver só no módulo;
e quando refatorar uma função que uma régua de profundidade mede, rode a régua:
número que **desce** sem o comportamento mudar é a régua perdendo alcance, não
melhora.

## Como está guardado hoje

- `mapa-da-trava.py --autoteste`, guardas 7 (`wait` conta como espera) e 8
  (chamada qualificada resolve só no módulo) — cada uma reprova com o defeito
  reposto, conferido em 01/10/2026;
- a catraca `rede-ou-espera` (0) foi **aposentada** e nasceu a
  `rede-ou-espera-2` em **11**;
- **o buraco que ficou**: a queda por profundidade não tem guarda — um ajudante
  novo em `nascendo::esperar` derruba o número para 8 e a catraca reprova como
  «DESCEU — BAIXE O TETO», o que convida a baixar. Quem vir a catraca descer
  sem o código esperar menos, desconfie da profundidade antes de baixar.
