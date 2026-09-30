# O irmao do excluir se acha pelo `remove_file`, nao pelo nome «excluir»

**Estado:** PENDENTE

## O que aconteceu

Pedido 591: `excluir_tabela` apagava os arquivos da tabela sem `fsync` da
pasta. A ordem veio com uma lista de irmaos por NOME: excluir schema, excluir
database, renomear, lixeira/esvaziar.

## O que eu concluí primeiro, e estava errado

Que os irmaos eram os da lista. Medido: **excluir schema e excluir database
nao existem** (nenhuma op no `servidor.rs`, nenhuma funcao no catalogo); o
renomear ja fazia o `fsync` das duas pastas desde o 467. Os irmaos reais eram
dois, e um deles nao estava na lista: o `esvaziar_lixeira` (apaga os volumes
do `.trash`) e a **fase 3 do expurgo da trilha** (apaga os volumes fechados do
`.lgpd`). Os dois apareceram varrendo `remove_file` em `phxsql-store/src`, e
nao `excluir`.

E o segundo morava atras de um comentario: o `Volumes::apagar_volume` dizia
«sem `fsync` do diretorio depois, e de proposito ... um ajudante unico e o
conserto certo». O ajudante passou a existir no 589 (`PorSincronizar`) e o
comentario continuou adiando -- comentario que se declara decidido e o motivo
de ninguem olhar de novo.

## O que a medição disse

`strace -y` (`catalogo::testes_excluir_vai_ao_disco::excluir_esvaziar_e_expurgar_vao_ao_disco`):
com o defeito reposto, cada uma das tres operacoes tem `unlink` bem-sucedido
na pasta do database e nenhum `fsync` dela antes da operacao seguinte; com o
conserto, o `fsync` da pasta vem depois do ultimo `unlink`. Quatro guardas
PROVADAS. `alcancam-fsync-2` 23 → 23 (o `fsync` sai da trava no servidor).

## A regra

Irmao de durabilidade se procura pela CHAMADA ao sistema (`remove_file`,
`rename`, `create_dir`), nao pelo nome da operacao.

## Como está guardado hoje

As quatro guardas do 591 em `bancada/guardas/catalogo.py`. O buraco que
fica: os `.json` do servidor (`rotinas.rs`, `visoes.rs`) gravam e apagam sem
`fsync` nenhum -- inclusive os gatilhos que o `excluir_tabela` apaga junto --,
e isso nao e o mesmo motor (nem o arquivo escrito vai ao disco), entao ficou
fora deste conserto.
