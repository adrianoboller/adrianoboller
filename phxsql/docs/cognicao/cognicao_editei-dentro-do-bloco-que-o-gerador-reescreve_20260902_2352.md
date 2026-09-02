# Editei dentro do bloco que o gerador reescreve — e ele comeu a edição

**Descoberto:** 02/09/2026, 23:52.
**Onde:** `docs/PENDENCIAS.md`, bloco `<!-- pedidos:contagem:inicio -->`;
gerador `docs/dossie/pagina-dos-pedidos.py`.

## 1. O que aconteceu

Acrescentei dois pedidos (168 e 169) ao `PENDENCIAS.md` ancorando a inserção na
**linha da contagem** — `**166 pedidos: 159 feitos…**` — porque era o texto
único e fácil de achar. Rodei o gerador, ele imprimiu `168 pedidos: 160 feitos,
5 parciais, 3 planejados`, conferi por `grep` que a tabela tinha 168 linhas, e
commitei.

O commit saiu com **duas linhas mudadas** no `PENDENCIAS.md`: só a contagem. Os
dois pedidos não estavam lá.

## 2. O que eu concluí primeiro, e estava errado

Que um gerador tinha «revertido o arquivo», e fui procurar qual dos cinco. Errado
nos dois sentidos: nenhum reverte nada, e o que apagou fez exatamente o que está
escrito na docstring dele.

A linha da contagem mora **dentro** do bloco delimitado por
`<!-- pedidos:contagem:inicio -->` e `:fim`. Ancorar nela pôs os dois pedidos
dentro do bloco. O `gravar_contagem` faz uma coisa só e a faz certo:
`md[:i] + ABRE + bloco + FECHA + md[j+len(FECHA):]` — substitui **tudo** entre as
marcas. As duas linhas foram embora.

O que torna isto pior que uma edição perdida é a **ordem**: o gerador **contou
antes de sobrescrever**. Ele leu 168 pedidos, escreveu `168`, e no mesmo passo
apagou as duas linhas que faziam 168. O arquivo ficou com **166 linhas de tabela
e a contagem dizendo 168** — um número gerado, e mais errado do que se ninguém
tivesse mexido. E o `grep` de conferência passou porque eu o rodei **antes** do
gerador.

## 3. O que a medição disse

| | linhas de pedido na tabela | a contagem dizia |
|---|---|---|
| depois da minha edição | 168 | 168 |
| depois do gerador | **166** | **168** |
| depois do conserto | 168 | 168 |

`git show --stat` do commit: `phxsql/docs/PENDENCIAS.md | 2 +-`. Duas linhas,
onde deviam ser duzentas.

## 4. A regra

**Conteúdo à mão entra FORA das marcas do gerador, e a âncora nunca é uma linha
que o gerador escreve.** Antes de inserir num arquivo gerado em parte, ache as
marcas e ancore no último item *da região à mão* — aqui, a linha do pedido 167.

E o corolário sobre a conferência: **conferir antes do gerador não confere
nada.** A prova de uma edição em arquivo gerado é a que roda **depois** de todos
os geradores.

## 5. Como está guardado hoje

* Os dois pedidos entraram de novo, ancorados na **linha do pedido 167**, com
  uma asserção no script de inserção de que a linha seguinte à âncora ainda não
  é o marcador `pedidos:contagem:inicio`.
* A conferência que vale passou a ser a de **depois**: contar `☑️/◐/☐` na tabela
  e comparar com a linha de contagem gerada. As duas batem: 168 = 160 + 5 + 3.
* Isto é a mesma família do «número digitado envelhece calado», mas o **alcance**
  é novo e é o que este arquivo registra: a lei dizia *todo número visível sai de
  um gerador*; o que faltava era *e a região que ele escreve não aceita
  companhia*.

**Onde o buraco ficou:** o `gravar_contagem` sobrescreve em silêncio. Ele
poderia recusar quando a região tem mais do que o bloco que ele mesmo produz —
seria a guarda que teria pego isto na hora, em vez de um commit depois. Não está
feito, e fica dito.
