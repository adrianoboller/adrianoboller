# A guarda no lugar errado causa o estrago que ela existe para impedir

**03/09/2026, 10:15** — descoberto medindo a réplica com
`--example sonda-replica-fk`, depois de fechar o primeiro buraco.

## 1. O que aconteceu

A conferência de chave estrangeira mora em `Table::inserir`, `Table::atualizar`
e `Table::excluir_de_vez`. A réplica aplica eventos chamando **essas mesmas
funções** (`Table::aplicar_evento`). Então, quando *chave declarada nasce
conferida* entrou, a réplica passou a conferir — sem que ninguém tivesse
decidido isso.

A replicação anda por **tabela**, cada uma com a sua posição no diário. Não
existe ordem global entre tabelas. Resultado, com um source que faz
`clientes.ins → pedidos.ins → clientes.alt` (a alteração cascateia até a filha):

* **ordem «mãe primeiro»**: a mãe chega e já muda de `1` para `2`; a inclusão da
  filha, que aponta para `1`, é **recusada**;
* **ordem «filha primeiro»**: a mãe ainda não chegou; **recusada** pelo mesmo
  portão.

Nos dois casos `pedidos` ficava com **0 dos 2** eventos, e a linha simplesmente
não existia na réplica. **A guarda de integridade referencial estava produzindo
órfãs — órfãs por ausência total da linha, que é a pior forma delas.**

O mesmo, por outra porta, no **bidirecional**: ele casa por chave e não por
rowid, então não passa pelo `aplicar_evento` e chama o `inserir` de sempre. Ali
a consequência é pior — o erro sobe pelo `?` do laço, `desde = lote.ate` nunca
executa, a posição não anda, e o mesmo lote volta na rodada seguinte. Para
sempre. **Não é uma linha perdida: é o par de servidores parado.**

## 2. O que eu concluí primeiro, e estava errado

Concluí que o defeito era **de ordem** e que o conserto era ordenar: aplicar as
tabelas mães antes das filhas dentro de cada lote, ou reter o evento recusado
numa fila até a mãe chegar.

Errado, e por um motivo que só apareceu escrevendo a ordem: **ordenar exige um
grafo de dependências entre tabelas que muda a cada `ALTER`**, e reter exige uma
fila persistente com política de expiração — as duas coisas para resolver um
problema que não existe. Não existe porque **a garantia já foi dada**: a origem
recusou o que tinha de recusar quando aceitou a escrita. O que a réplica precisa
garantir não é integridade, é **fidelidade** — e fidelidade ela já confere, por
SHA-256 de cada linha.

Conferir duas vezes não soma as duas garantias. **Troca a segunda pela
primeira.**

## 3. O que a medição disse

Com a sonda, antes e depois, nas três ordens de entrega:

| ordem | antes | depois |
|---|---|---|
| mãe primeiro | `pedidos 0 eventos (source 2)`, linha inexistente | 2 de 2, `Int(2)` |
| filha primeiro | `pedidos 0 eventos (source 2)` | 2 de 2, `Int(2)` |
| entrelaçada | 1 evento **a mais** que o source (cascata refeita) | 2 de 2, `Int(2)` |

E o custo do que **não** se mexeu: a escrita local continua conferindo, medida
no mesmo teste — `a_marca_de_replica_nao_vaza_para_a_escrita_local`. A marca é
de **um evento**, não do handle.

## 4. A regra

**Guarda copiada para a camada errada não protege: causa.** Antes de deixar uma
conferência valer num caminho novo, pergunte *quem já deu esta garantia* — se
alguém já deu, repeti-la aqui não a reforça; substitui a garantia dele pela
sua, que tem menos informação.

E o par que decide onde ela vale: **quem JULGA é quem aceita a escrita pela
primeira vez; quem APLICA o que outro já julgou não julga de novo.**

## 5. Como está guardado hoje

* `Table::julga_integridade` (`crates/phxsql-store/src/table.rs`) é a pergunta
  única, com os três números da medição no comentário dela. Os três pontos de
  conferência a consultam; não há um quarto lugar que decida a mesma coisa.
* A marca liga e desliga num **par só**, no `aplicar_evento`, com o trabalho num
  interno — um `return` no meio deixaria o handle sem portão para a escrita
  local seguinte, e portão que se apaga sozinho é pior que portão nenhum, porque
  ninguém procura por ele. Há guarda provada para isso
  (`marca-de-replica-fica-acesa`).
* O bidirecional entra pelo store, por três métodos com o motivo escrito
  (`inserir_replicado`, `atualizar_replicado`, `excluir_de_vez_replicado`), e no
  servidor mudam três chamadas — superfície mínima num arquivo compartilhado.
* Seis guardas provadas cobrem os dois caminhos, e cada teste confere os **dois
  lados no mesmo corpo**: o caminho local recusa, o replicado aceita. É a
  diferença entre eles que é a garantia, e um teste que só olhasse um lado
  passaria com o portão inteiro apagado.
* **Onde o buraco ficou:** não há conferidor que liste «quem chama
  `Table::inserir`». O bidirecional só apareceu porque eu fui atrás dos
  chamadores um a um; uma varredura por `aplicar_evento` não o acharia, porque
  ele não passa por lá.
