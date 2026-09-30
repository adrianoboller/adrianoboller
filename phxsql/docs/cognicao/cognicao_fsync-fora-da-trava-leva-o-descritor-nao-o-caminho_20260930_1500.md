# O `fsync` fora da trava leva o DESCRITOR, não o caminho

**Estado:** PENDENTE

## O que aconteceu

Pedidos 552 e 579, `crates/phxsql-store/src/backup.rs`. Para tirar o `fsync`
de baixo da trava de dados (513/524), o backup passou a devolver a lista de
CAMINHOS escritos, e o `sincronizar_copias` reabria cada um para o `fsync`.
Isso tirou o `fsync` da trava e, junto, deixou o nucleo livre para despejar o
inode entre o `close` e o `open`, levando junto o erro de *writeback*
(fsyncgate). A remocao do `backup.json` velho (577) ficou sem `fsync` do
diretorio pelo mesmo motivo: roda sob a trava.

## O que eu concluí primeiro, e estava errado

Que a fronteira da trava obrigava a devolver CAMINHOS, porque «o `File` e de
quem escreveu». Nada obriga isso: o `File` atravessa a fronteira como qualquer
valor, e a trava protege a leitura de `raiz`, nao o descritor do destino. O
unico custo real e segurar descritor aberto, e esse se mede (a folga do
`/proc/self/limits` menos o `/proc/self/fd`) em vez de servir de motivo para
reabrir tudo.

E a segunda: que a prova do 579 cabia na arma `falha_de_teste`. Nao cabe. A
arma casa por PREFIXO de caminho, e o `fsync` do manifesto novo casa o mesmo
prefixo que o da pasta. Com o defeito reposto, a arma derrubava o manifesto e
o resultado era identico. A ordem das chamadas so o `strace` ve.

## O que a medição disse

Sob `strace -f`, com o conserto: cada copia e o manifesto abrem com sucesso
UMA vez, e o `fsync` cai no mesmo numero de descritor sem `close` no meio. O
`fsync` de um descritor da pasta vem entre o `unlink` do manifesto velho e o
`openat` do novo. Com cada defeito reposto, o teste correspondente cai: as 5
guardas novas ficaram PROVADAS no provador. Uma armadilha do registro: o
`strace` alinha o `= ret` em coluna (`fsync(3)      = 0`), entao procurar
`") = "` nao acha o `fsync`, e o teste reprovaria o conserto.

## A regra

Quando um passo sai de baixo da trava, faça atravessar a fronteira o recurso
que o passo usa, e nao um nome pelo qual ele se reabre.

## Como está guardado hoje

`crates/phxsql-store/tests/fsync-no-descritor-que-escreveu.rs` (5 testes) e 5
guardas em `bancada/guardas/catalogo.py`. O buraco fica em dois lugares. As
copias alem do teto de descritores ainda reabrem. A janela entre o `unlink`
sob a trava e o `fsync` da pasta nao foi medida, porque pede queda real.
