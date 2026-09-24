# LZMA: o preço era metade da diferença — a busca era a outra metade

## 1. O que aconteceu

O codificador LZMA2 do PhxZip saiu +12,4% maior que o `7z -mx5 -mf=off` na
mesma entrada (1.619.709 bytes). Escrita a análise ótima por preço
(`crates/phxzip/src/lzma/otimo.rs`), caiu para +5,8% — e parou ali.

## 2. O que eu concluí primeiro, e estava errado

Que a diferença inteira era a análise ótima: o documento da crate dizia «a
diferença tem causa conhecida: o 7-Zip escolhe pelo preço». Depois do preço,
as duas hipóteses seguintes ainda eram sobre o preço (refazer as tabelas mais
vezes, olhar mais longe) — e morreram com −0,07% e −0,005%.

## 3. O que a medição disse

Separar a entrada respondeu: texto sozinho +8,9%, binário sozinho +2,1%. No
texto, o nível 9 daqui (profundidade 128) chegava perto do 7-Zip e o 5
(profundidade 32) não — a **busca** perdia os casamentos longos, e preço
nenhum escolhe casamento que a busca não achou. A cadeia de dispersão de 3
bytes, num texto cheio de «| pedido » e «do dono», enche de candidatos que
casam 3 bytes e param. Dispersão de 4 bytes: nível 5 foi a +3,6%, e mais
rápido em todos os níveis.

## 4. A regra

Quando um compressor perde, separe a entrada por tipo antes de atacar o
algoritmo: o mesmo número agregado esconde causas diferentes em texto e em
binário. E hipótese que só mexe no escolhedor não ajuda se o buscador não
entregou a opção.

## 5. Como está guardado hoje

As sete hipóteses, com o número de cada uma, estão na tabela do
`docs/PHXZIP.md` §3. O teste `o_preco_ganha_do_guloso_em_texto` cai se o
planejador deixar de ser chamado (medido: 20.342 = 20.342 com ele desligado).
Não há guarda para a dispersão de 4 bytes: uma volta a 3 bytes só apareceria
na bancada, como número pior — e a bancada do PhxZip ainda não grava
`resultados.json`. Buraco registrado.
