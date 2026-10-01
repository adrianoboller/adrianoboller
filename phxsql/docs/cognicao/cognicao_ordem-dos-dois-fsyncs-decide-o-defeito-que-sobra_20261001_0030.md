# A ordem de dois `fsync` que não cabem juntos decide qual defeito sobra na queda

**Estado:** PENDENTE

## O que aconteceu

Pedido 595. O `excluir_tabela` apaga os arquivos da tabela e tira os gatilhos
dela do `gatilhos.json`. São duas mudanças em disco que não podem ser
atômicas juntas (uma é `unlink` na pasta, a outra é a troca de um arquivo), e
as duas precisam de `fsync` fora da trava global. Entre os dois `fsync` existe
uma janela de queda, qualquer que seja a ordem.

## O que eu concluí primeiro, e estava errado

Que bastava pôr os dois `fsync` depois de soltar a trava, «na ordem do
código»: primeiro o `PorSincronizar` da tabela (que já existia, 591), depois
o `gatilhos.json`. Nessa ordem, uma queda entre os dois deixa a tabela fora do
disco e o gatilho DENTRO. É exatamente o órfão que o pedido nomeia: ele
dispararia sobre a homônima que alguém criasse depois.

## O que a medição disse

O `strace -y` do teste
`servidor::testes_gatilhos::cadastro_vai_ao_disco_595::o_cadastro_vai_ao_disco_antes_da_resposta`
reprova a ordem invertida («o sumiço da tabela foi ao disco ANTES do
gatilhos.json») e aprova a escolhida. Na ordem escolhida (cadastro primeiro), a
queda entre os dois devolve, no máximo, a tabela sem os gatilhos de uma
exclusão que o cliente nunca ouviu terminar. É dado incompleto de uma ordem
não confirmada, e não escrita no dado de outro.

Não medido: a queda em si. O que se prova é a ordem das chamadas.

## A regra

Quando dois `fsync` não cabem numa operação atômica, escreva qual estado a
queda entre eles deixa, nas duas ordens, e escolha a ordem cujo resíduo não
age sozinho sobre dado alheio.

## Como está guardado hoje

Guarda `gatilho-orfao-na-queda-do-excluir-tabela` (`bancada/guardas/catalogo.py`),
com a troca de ordem reposta. `docs/FORMATO.md` §15 registra a ordem e o
motivo. Buraco: o resíduo da ordem escolhida (tabela que volta sem os
gatilhos) não tem guarda. Ele só se fecharia com uma marca de intenção
gravada antes das duas mudanças, e essa marca não existe.
