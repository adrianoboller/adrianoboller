# O `openat` que a `std` não tem sai do `/proc/self/fd/N/nome`

**Estado:** PENDENTE

## O que aconteceu

Pedidos 568/569/570: o backup gravava através de link numa pasta do MEIO do
destino (`copias/loja -> dados/rh`), no arquivo de outro dono que já estava no
nome, e parava numa FIFO trocada entre o `lstat` e o `open`. O conserto do 542
dizia, no comentário do `util.rs`, que fechar a pasta do meio «pede `openat`, que
a `std` não dá», e deixou para pedido próprio.

## O que eu concluí primeiro, e estava errado

Que só havia duas saídas: FFI com `unsafe` (`extern "C" { fn openat(...) }`) ou
`lstat` de cada componente seguido de `open` pelo nome — que deixa a janela da
troca aberta, justamente o 570. As duas eram piores que a terceira.

## O que a medição disse

No Linux, abrir `/proc/self/fd/N/nome` resolve o `N` pelo DESCRITOR — o
diretório que se abriu, mesmo que o nome dele mude depois — e só o `nome` por
nome. Com `O_NOFOLLOW` (via `OpenOptionsExt::custom_flags`), o link nesse `nome`
dá `ELOOP`: é o `openat(N, nome, O_NOFOLLOW)` sem `unsafe`. Provado contra o
núcleo: `util::tests::sem_seguir_recusa_link_e_fifo_de_verdade` (`ELOOP`,
`ENXIO`, `ENOTDIR`) e as três provas de
`crates/phxsql-store/tests/destino-do-backup-sem-atalho.rs`; a da FIFO deu
538.805 voltas em 4 s sem parar, e com o defeito reposto a guarda estourou o
prazo de 20 s do teste (25,8 s na rodada).

## A regra

Quando a `std` não tem a chamada `*at`, alcance o nome pelo descritor da pasta
(`/proc/self/fd/N/nome`) e decida pelo `fstat` do que abriu — nunca pelo `lstat`
do nome seguido de `open`.

## Como está guardado hoje

`util::Pasta` (`crates/phxsql-store/src/util.rs`) e as guardas
`backup-atravessa-link-na-pasta-do-meio`, `backup-escreve-no-arquivo-de-outro-dono`,
`fifo-trocada-na-janela-para-o-backup` e `copia-reaberta-pelo-nome-no-fsync`.
O buraco: fora do Linux (e sem `/proc`) a `Pasta` recua para o `lstat` por
componente, e a janela da troca fica; e as constantes `O_*` só existem nas
arquiteturas conferidas no `fcntl.h` (`util::bandeiras`).
