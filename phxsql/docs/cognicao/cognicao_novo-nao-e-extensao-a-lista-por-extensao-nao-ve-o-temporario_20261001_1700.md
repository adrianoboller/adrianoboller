# `.novo` não é extensão: a lista por extensão não vê o temporário

**Estado:** PENDENTE

## O que aconteceu

Pedido 618 (revisão SEC sobre o 364). A redeclaração do índice de texto monta
`<tabela>.fts.novo` na FASE A; um pânico depois do `sincronizar` o deixava no
disco. O `excluir_tabela` decide o que apagar por `pertence(arquivo, tabela,
ext)` sobre `EXTENSOES_TODAS` — e `clientes.fts.novo` não termina em nenhuma
extensão. A tabela sumia e o vocabulário da coluna indexada ficava.

## O que eu concluí primeiro, e estava errado

Que o furo era só do `.fts.novo` e que bastava pôr `"fts.novo"` na lista. Mas o
irmão (quem chama as mesmas funções na mesma ordem: a FASE A do 422) deixa
também `clientes.reg.novo` — cópia do `.reg` INTEIRO —, e o `renomear_tabela`
usa o mesmo `pertence`. Uma extensão a mais teria fechado um dos três arquivos
num dos dois caminhos.

## O que a medição disse

Com o defeito reposto, o `excluir_tabela` deixou `["clientes.fts.novo",
"clientes.reg.novo"]` e o `renomear_tabela` deixou os mesmos dois sob o nome
velho. Com o sufixo `.novo` tratado como «arquivo desta tabela», zero sobras
nos três testes.

## A regra

Quem leva a tabela INTEIRA (excluir, renomear) leva também os `*.novo` dela;
quem copia ou inventaria, não — arquivo pela metade não é tabela.

## Como está guardado hoje

`pertence_ou_sobra` em `phxsql-store/src/catalogo.rs`; guardas
`novo-orfao-sobrevive-ao-excluir-tabela` e `fts-ao-lado-sobrevive-a-abertura`.
**O buraco que fica:** os `*.novo` do `.reg` de uma FASE A morta continuam no
disco enquanto a tabela vive (decisão antiga do `terminar_troca_interrompida`:
«leitura não apaga»), e o `duplicar_tabela` não os copia — numa tabela
paginada com troca decidida e não terminada, a cópia sai com volumes
misturados.
