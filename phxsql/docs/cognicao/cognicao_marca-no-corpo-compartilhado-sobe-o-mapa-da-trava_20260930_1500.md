# A marca da cascata no corpo compartilhado do `atualizar` sobe o mapa da trava; na porta, não

**Estado:** PENDENTE

## O que aconteceu

Pedido 563: levar a marca `.tx` da cascata do `ao_alterar` ao `Table::atualizar`
do embutido. A primeira versão pôs o passo da marca (`gravar_a_marca_da_cascata`
→ `gravar_marca` → `sync_all`) **dentro** do `atualizar_com_maes_opt_conferindo`,
o corpo que todo `atualizar` compartilha, ligado por um modo de três casos
(`PelaMarca`, `EmLinha`, `Nenhuma`). A suíte passou, a prova de `SIGKILL` deu
`[6, 6]` — e a catraca `alcancam-fsync-2` do `mapa-da-trava.py` subiu de
**23 para 25**.

## O que eu concluí primeiro, e estava errado

Que era falso positivo e bastava explicar: as duas seções novas vinham de
`aplicar_evento → aplicar_evento_interno → atualizar_com_maes_opt_conferindo →
gravar_a_marca_da_cascata`, e a replicação chama o corpo com o modo `EmLinha`,
que nunca entra no ramo da marca. Verdade em execução, mas o mapa resolve **por
nome** de propósito (erra para o lado seguro), e «explique no relatório» seria
subir a catraca com motivo escrito — o que a lei proíbe.

## O que a medição disse

O defeito era de desenho, não do mapa: um corpo compartilhado carregando o
passo de UMA porta. Partido em duas metades — `planejar_a_alteracao` (tudo o que
recusa, nada gravado) e `gravar_a_alteracao` (a mãe e os observadores) —, só o
`Table::atualizar` público costura a marca entre elas, e os outros caminhos
(passada, recuperação, replicação) chamam as metades sem passar perto dela.
Mapa de volta a **23**, sem mudar régua nenhuma; a mesma suíte e a mesma prova
`[6, 6]` / mutante `[6, 5]`.

## A regra

Passo que só uma porta dá mora na porta, e não no corpo que todas compartilham:
quando um modo novo aparece num corpo comum, parta o corpo onde o passo entra.

## Como está guardado hoje

Pela própria catraca `alcancam-fsync-2` (23), que acusaria o passo de volta no
corpo; e pelas guardas `cascata-do-embutido-sem-marca` e
`cascata-em-voo-so-no-aplicar` do catálogo, que citam o trecho da porta.
