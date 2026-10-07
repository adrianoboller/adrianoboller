# A falha forjada no `fsync` não rasga o slot do `.seq`: a prova do 665 é na escrita

**Estado:** PENDENTE

## O que aconteceu

Pedido 665: o `.seq` andava `geracao`/`proximo` antes de gravar, e duas
gravações que falham seguidas podiam invalidar os dois slots. Para o teste
adverso havia uma injeção pronta, `falha_de_teste::Onde::Fsync`, e foi a
primeira candidata.

## O que eu concluí primeiro, e estava errado

Que armar `Onde::Fsync` duas vezes reproduziria o defeito. Não reproduz: o
`write` do slot já aconteceu inteiro quando o `fdatasync` forjado devolve EIO,
então os dois slots ficam **válidos** (só adiantados), e a reabertura lê a
geração mais alta sem reclamar. Pior: a primeira recusa de `fsync` marca o
diretório em `RECUSADOS`, e toda gravação seguinte naquela pasta recusa —
o teste mediria a trava do 509, não o slot.

## O que a medição disse

Com a injeção nova `Onde::GravacaoDaSequencia` (metade do slot escrita, e
ENOSPC) e o avanço antes da gravação reposto, a reabertura recusa:
`[SP000010] arquivo corrompido: ... o CRC-32 nao confere` — os dois slots
rasgados. Com o conserto, as duas falhas caem no mesmo slot alternado e a
reabertura lê `proximo = 3` depois de entregar 2.

## A regra

Prove o defeito de slot alternado com a falha que **estraga o slot** (escrita
rasgada), não com a que só **atrasa a durabilidade** (`fsync`).

## Como está guardado hoje

`crates/phxsql-store/src/sequencia.rs::duas_gravacoes_que_falham_seguidas_nao_rasgam_os_dois_slots`
e a guarda `seq-avanca-antes-de-gravar` no `bancada/guardas/catalogo.py`
(RED medido à mão, ainda não provada pelo provador).
