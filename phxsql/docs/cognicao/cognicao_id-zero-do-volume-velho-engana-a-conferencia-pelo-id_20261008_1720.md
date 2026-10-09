# O id zero do volume velho engana a conferência pelo id

**Estado:** PENDENTE

## O que aconteceu

Pedido 710 (F2): a recuperação da marca do `COMMIT` passou a conferir as duas faces — o slot
no `.reg` **e** o evento no diário — e, com o `tx` que a marca v7 traz, procura no diário os
eventos da linha com aquele id, lendo de trás para a frente até o primeiro id menor (o id só
cresce no diário de uma tabela, pedido 684).

## O que eu concluí primeiro, e estava errado

Que «parar no primeiro id menor» era seguro em todo diário. Não é: um volume da versão 2 ou 3
(anterior ao 676) continua sendo o ativo de uma tabela antiga até virar, e grava o evento com
id **zero**. Zero é menor que qualquer id, a leitura parava no primeiro evento, achava o
diário «sem nada desta marca» e completava a inclusão **de novo** — o evento em dobro, numa
marca cuja passada tinha terminado, no arranque de toda tabela antiga.

## O que a medição disse

`crates/phxsql-store/src/log.rs::a_marca_com_id_no_diario_sem_id_nao_duplica_a_inclusao`:
diário da versão 2 com a inclusão (id 0), marca v7 com o id da passada; sem a guarda o
diário sai com **2** eventos, com ela **1**.

## A regra

Antes de decidir pelo id do evento, confira que o volume grava o id: zero não é «menor», é
«não sei», e decisão tomada sobre «não sei» volta o caminho antigo (aqui, a inclusão pelo
rowid).

## Como está guardado hoje

`diario_da_linha` (`phxsql-store/src/marca.rs`) devolve `None` ao achar evento com id zero,
e a `so_o_diario` cai na conferência pelo rowid. O teste acima trava os dois sentidos.
