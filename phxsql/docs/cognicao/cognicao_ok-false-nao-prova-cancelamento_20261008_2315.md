# `ok:false` não prova cancelamento: outro portão também recusa

**Estado:** PENDENTE

## O que aconteceu

O caso web `botoes-da-telemetria-viva` (pedido 741) caiu em `[escuro]` na bateria
de 08/10/2026, com a frase «a carga nunca apareceu como operação em curso».
Medido sob carga (load 8 a 24, 4 `yes` a mais): **10 de 10** corridas caíram. O
caso dava 20 voltas ao `Atualizar agora`, cerca de 11 s, e com a máquina carregada
o servidor levava de 60 a 90 s só para ler o pedido de 46 MB.

A causa que pesava mais estava no diagnóstico acrescentado à falha: a carga de
1,6 milhão de linhas respondia `ok:false` com **«limite excedido: … 204800000
bytes no diário, acima do teto de 67108864»**. Desde o pedido 686, carga acima
de 64 MiB previstos no diário é recusada no fim da conversão. Isso deixava o
caso com dois furos:

- o ramo «não cancelável» exigia `ok:true` e nunca podia passar;
- o ramo «encerrando» exigia `ok:false` e passava por engano, porque a recusa do
  teto também é `ok:false`.

## O que eu concluí primeiro, e estava errado

Concluí que o defeito era só de relógio: 20 voltas contra uma carga lenta.
Esperar o evento com paciência longa resolveria. Com a espera por evento no
lugar e o **defeito reposto** (a marca de encerrar que não se põe), o caso
**aprovou**. O clique caía com a carga ainda na fila, a resposta vinha
«marcada», e «marcada» admite os dois desfechos.

## O que a medição disse

- Antes do conserto: 10/10 caíram sob carga, as 20 voltas gastando entre 10.382 e
  13.032 ms. A carga respondia com a recusa do teto entre 91 e 112 s depois do
  envio.
- Com 480 mil linhas (cerca de 61 MB previstos), espera pelo evento e o clique só
  com o servidor dizendo `cancelavel`:
  - defeito da marca reposto: **3/3 caem**, com «encerrando … e a carga acabou em
    terminou»;
  - defeito do botão que nunca arma reposto: cai com «a carga JA tinha
    respondido ok=true»;
  - sem defeito, sob a mesma carga: **20/20** passam, todas com a resposta forte
    «encerrando».

## A regra

O desfecho de uma operação se confere pelo **nome** do erro (`CANCELADO`), nunca
por `ok:false`. E o clique que testa uma promessa espera a fase em que a promessa
é a forte, e não a primeira em que o botão aparece.

## Como está guardado hoje

Está em `testes-web/casos/41-botoes-da-telemetria-viva.mjs`. A carga cabe no teto
do diário, a espera dura enquanto a carga não respondeu (120 s só de
desistência), e o fim se confere por `resposta.nome`. O tamanho da carga depende
dos 128 bytes por linha medidos para esta tabela. Se o teto ou a conta do 686
mudarem, a falha agora nomeia a recusa em vez de aprovar calada. Mas **nenhum
conferidor** acha outros testes que aceitem `ok:false` como prova de um erro
específico: esse buraco continua aberto.
