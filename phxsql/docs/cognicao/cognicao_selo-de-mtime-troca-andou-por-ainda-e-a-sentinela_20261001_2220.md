# Perguntar «ainda é a sentinela?» em vez de «o `mtime` andou?» fecha o tique grosso sem mudar o formato

**Estado:** PENDENTE

## O que aconteceu

Pedido 634. O retrato da FASE A (`reg.rs`, `retratar`) guardava tamanho e
`mtime` de cada volume, e a FASE B conferia que nada tinha mudado. A
atualização no lugar não muda tamanho nem byte do cabeçalho (medido no 427:
zero dos 128), então o `mtime` era o único sinal dela — e com o tique grosso
(FAT 2 s, HFS+/NFS 1 s) a escrita no mesmo tique da anterior não o muda. A
linha 45 voltava de «NOVO» para o valor velho depois da troca, sem erro.

O conserto: no retrato, com a trava na mão, `File::set_modified` grava em
cada volume o `mtime` 1980-01-02 00:00:00 UTC; a FASE B confere que ainda é
o valor que o sistema de arquivos guardou ao selar. Troca que não acontece
devolve o original, só a quem ainda tem a sentinela (`Selos`, no `Drop`).

## O que eu concluí primeiro, e estava errado

Que a simulação do tique grosso do 427 servia para provar o 634. Ela
devolvia ao volume o `mtime` **de antes da escrita, seja ele qual for** — e
com o selo o «de antes» é a própria sentinela. O teste passaria com e sem o
conserto: um tique que apaga até a escrita que cai a 46 anos de distância não
é tique, é relógio parado. Foi preciso separar dois adversários no teste:
`Relogio::Parado` (o do 427, que isola o cinto dos ausentes) e
`Relogio::Grosso` (a escrita no MESMO tique do valor anterior não muda; em
outro tique, muda) — e só o segundo é o mundo real.

## O que a medição disse

- Com o selo tirado do `retratar_existentes`: **3 de 8** testes do arquivo
  caem (as duas FASES A irmãs e o que confere o selo visível); com ele, 8/8.
- Com a devolução do original tirada do `Drop`: **1 de 8** cai
  (`a_troca_abortada_devolve_o_mtime_original_634`).
- Custo: um `open` e dois `fstat` por volume no retrato, numa operação que
  copia a tabela inteira.

## A regra

Quando um sinal tem granularidade grossa, não compare «antes» com «depois»:
plante um valor que nenhum evento real produz e pergunte se ele continua lá.

## Como está guardado hoje

- `crates/phxsql-store/tests/volume-que-nasce-na-fase-a.rs`, os quatro
  `*_634`.
- Guardas `retrato-da-fase-a-sem-selo-634` e
  `selo-do-retrato-nao-devolve-o-mtime-634`.
- `docs/FORMATO.md`, na seção da FASE A fora da trava.
- **O buraco:** NTFS e FAT reais não foram medidos nesta máquina — a
  sentinela foi escolhida representável nos dois (ano 1980, segundo par, em
  qualquer fuso de -12 h a +14 h), mas isso é leitura da especificação, não
  medida.
