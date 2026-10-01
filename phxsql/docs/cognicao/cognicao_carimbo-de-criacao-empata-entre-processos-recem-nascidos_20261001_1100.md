# O carimbo de criação empata entre dois `phxsqld` recém-nascidos — a prova de duas origens precisa de histórias diferentes

**Estado:** PENDENTE

## O que aconteceu

Pedido 416 (01/10/2026). A exclusão replicada passou a conferir o `rowstamp` da
imagem de antes contra o da linha que mora no rowid — a mesma
`conferir_identidade` da alteração (pedido 405). A prova pelo soquete
(`crates/phxsql-server/tests/imagem-pela-politica-do-servidor.rs`) sobe dois
`phxsqld` como duas origens, cada um grava a sua linha no rowid 1 de
`loja.clientes`, e a exclusão da origem B é empurrada pelo `aplicar` para um
destino que guarda a linha da origem A.

Com o conserto inteiro no lugar, o teste **continuou vermelho**: o destino
apagou a linha de A com `aplicados: 1`.

## O que eu concluí primeiro, e estava errado

Que o conserto não estava ligado — a imagem não estava indo na exclusão, ou a
conferência não rodava naquele caminho. As duas outras provas do mesmo arquivo
(a réplica fiel recebendo exclusão **com** imagem) desmentiam a primeira
metade, e o braço `Exclusao` do `aplicar_evento_interno` é o único caminho do
`aplicar`.

## O que a medição disse

O carimbo é um contador do **processo** (`phxsql-store/src/no.rs`,
`ULTIMO_CARIMBO` nasce em 0). Dois `phxsqld` recém-nascidos, cada um gravando a
primeira linha da vida dele, emitem **o mesmo número: 1**. A conferência viu
1 == 1 e deixou passar — exatamente o alcance que o documento do 405 já
escrevia («pega divergência, não prova acordo») e que a prova do 405 nunca
encontrou, porque ela abria as duas origens como dois `Table` **no mesmo
processo**, onde o contador é compartilhado e os números saem distintos por
construção.

Andando a história de B cinco linhas numa tabela vizinha antes, os carimbos
saem 1 e 6, e a exclusão para com «replica divergiu … carimbo».

## A regra

Prova de identidade entre servidores se escreve com **processos separados e
histórias diferentes** — duas origens no mesmo processo compartilham o
contador e escondem o empate que dois servidores de verdade produzem no
primeiro dia.

## Como está guardado hoje

- O teste dá à origem B uma história própria, com o motivo escrito no
  comentário.
- A guarda `exclusao-replicada-sem-conferir-o-carimbo`
  (`bancada/guardas/catalogo.py`) repõe o braço sem conferência e exige a
  queda.
- **O buraco fica nomeado:** dois caixas que nasceram no mesmo dia e gravaram o
  mesmo número de linhas empatam o carimbo, e a conferência os deixa passar.
  Fechar isso pede identidade de nó no carimbo — mudança de formato, do papel C,
  sem pedido aberto nesta frente.
