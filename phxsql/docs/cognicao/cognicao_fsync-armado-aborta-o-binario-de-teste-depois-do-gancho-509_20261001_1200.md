# O `fsync` recusado de propósito aborta o binário de teste inteiro, se um `Servidor` já nasceu nele

**Estado:** PENDENTE

## O que aconteceu

Frente dos pedidos 534/535. A primeira prova do `promover` armava
`falha_de_teste::armar(dir, Onde::Fsync, 1)` — o mesmo molde de
`config.rs::gravar_privado_so_responde_depois_do_fsync_do_diretorio`. Rodando
`cargo test -p phxsql-server --lib -- cluster:: bidirecional:: gravar_privado`,
o binário caiu com SIGABRT, e quem disparou foi o teste **antigo** do
`gravar_privado` (`dblink.json`), não o novo.

O motivo: `Servidor::novo` registra `sincronia::ao_recusar(abort)` (pedido 509)
num `OnceLock` do processo. Depois que QUALQUER teste do mesmo binário sobe um
`Servidor`, todo `fsync` recusado com gancho — inclusive o armado de propósito —
derruba o processo.

## O que eu concluí primeiro, e estava errado

Que bastava copiar o molde do teste do `gravar_privado`, porque ele passa na
suíte inteira. Ele passa por **ordem**: na corrida que abortou, o filtro juntou
testes que sobem `Servidor` com ele no mesmo processo.

## O que a medição disse

- Filtro `cluster:: bidirecional:: gravar_privado`: SIGABRT, 1 de 1 corridas.
- `./portoes.sh` inteiro no mesmo dia: verde. O teste antigo é dependente de
  ordem, e não está quebrado na corrida de sempre.

## A regra

No binário de teste do `phxsql-server`, falha de `fsync` provocada se prova em
**processo filho** (o molde do `fsync_recusado_derruba_o_processo_e_a_marca_fica`),
ou por outro caminho de erro que não passe pelo gancho — nunca por `armar(Fsync)`
no próprio processo.

## Como está guardado hoje

O teste novo (`cluster::testes::promover_que_nao_chega_ao_disco_nao_libera_escrita`)
faz a gravação falhar com uma pasta no nome do temporário, e a ordem
`fsync → rename → fsync da pasta` se prova pelo `strace` em filho
(`cluster::testes::o_estado_vai_ao_disco_pela_troca_duravel`). **O buraco que
fica:** `config.rs::gravar_privado_so_responde_depois_do_fsync_do_diretorio`
continua armando o `fsync` no processo, e aborta o binário quando a ordem junta
ele com um `Servidor`.
