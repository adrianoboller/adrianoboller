# O `timeout` do tokio não solta uma leitura de FIFO, e o `fmt` de outra frente move o defeito reposto

**Estado:** FRUTÍFERO

**Evidência:** `tests/fluxo_onda3b.rs::binario_so_de_arquivo_regular_dentro_de_binarios`, na
árvore desta rodada (sobre `211b9cd6`). Com o `if !md.is_file()` do `fluxos::ler_binario`
reposto, a primeira versão do teste — `tokio::time::timeout(5 s, retomar(..))` — **não caiu**:
ficou presa e o `timeout 300` do terminal matou a corrida (código 143). A versão com a retomada
numa thread própria e `recv_timeout(5 s)` cai em 5,0 s com «a retomada ficou presa no FIFO»;
com o conserto, passa em 0,5 s.

## O que aconteceu

A retomada lia o binário de um item com `std::fs::read`. Um FIFO no lugar do arquivo prende a
leitura para sempre, e a retomada roda no laço do servidor.

## O que eu concluí primeiro, e estava errado

Que `tokio::time::timeout` em volta da retomada provaria a trava como um teste que CAI. Ele só
cancela no próximo `.await`; a leitura síncrona prende a thread do runtime, e o `timeout` nunca
tem a vez. O teste não falhava — pendurava, e teste que pendura não é RED, é corrida perdida.

E o segundo erro, no mesmo dia: o script de repor defeitos casava o texto exato do fonte, e
**outra frente rodou `cargo fmt --all` entre o repor e o build** — o trecho reposto foi
reformatado (um `Html(..)` sem `.into_response()` passou a não compilar) e as buscas seguintes
do script falharam por formatação, não por conteúdo.

## A regra

1. O que prende uma thread se prova com prazo FORA do runtime (thread própria + `recv_timeout`),
   nunca com o `timeout` do próprio runtime.
2. Em rodada com frentes paralelas no mesmo workspace, o script de RED salva o original logo
   antes de repor, casa o texto **depois** do `fmt`, e restaura escrevendo o conteúdo (mtime
   novo; `cp -p` faria o cargo achar que nada mudou).

## Como está guardado hoje

O teste do FIFO usa thread + `recv_timeout`; o `ler_binario` olha o `symlink_metadata` antes
de abrir ou canonicalizar (nem segue link, nem abre FIFO).
