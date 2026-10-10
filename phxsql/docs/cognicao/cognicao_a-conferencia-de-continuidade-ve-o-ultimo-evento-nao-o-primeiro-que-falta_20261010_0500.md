# A conferência de continuidade vê o último evento, não o primeiro que falta

**Estado:** PENDENTE

## O que aconteceu

Pedido 299, F2: a tabela de continuidade rompida passou a segurar o database
inteiro a partir da primeira transação da origem que a toca. A primeira versão
tirava essa transação do evento que a conferência já compara — o `posicao - 1`
da origem (`Servidor::diario_local_continua`).

## O que eu concluí primeiro, e estava errado

Que o evento conferido era o primeiro que faltava aqui. Ele é o primeiro só
quando a réplica divergiu por **uma** escrita. Com duas escritas locais em
`pagamentos`, a posição local é `P + 2`, a conferência compara o evento
`P + 1` da origem — o **segundo** que falta —, e a barreira saía da transação
dele: a venda do evento `P` passava inteira em `vendas` e `itens`, sem o
pagamento. O mesmo defeito que a F2 existe para fechar, pela porta do lado.

## O que a medição disse

Com a barreira tirada do `posicao - 1` (a busca começando em `alcance - 1`),
`venda-inteira-na-ruptura-da-replica::a_tabela_rompida_segura_a_transacao_inteira_e_as_seguintes`
dá `(1, 4, 0)` para a venda 3. Com a busca binária do prefixo comum
(`Servidor::barreira_da_rompida`, log2 da tabela em idas e voltas, uma vez
por estado), dá `(0, 0, 0)`.

Dois achados do mesmo dia, do mesmo formato — o que a casa confere não é o que
ela acha que confere:

- `Table::aplicar_evento` conferia o rowid da inclusão **depois** de gravar:
  a linha entrava no slot errado antes da recusa (medido no HEAD `edbd6180`:
  `(1, 4, 1)` com o pagamento no rowid 3).
- `PhxError::com_nota` devolvia `Corrompido` como veio: a nota do 713 («a
  marca fica no disco e o arranque a completa») nunca chegou a ninguém na
  família de erro em que ela mais importa.

## A regra

Antes de tirar uma decisão de uma conferência que já existe, pergunte **o que
ela compara de fato** — um evento, o último, a contagem — e se a decisão nova
precisa de outra coisa.

## Como está guardado hoje

Pelas guardas `299-barreira-so-do-ultimo-evento`,
`299-rowid-conferido-depois-de-gravar` e `299-nota-do-corrompido-calada`
(`bancada/guardas/catalogo.py`). Fica de fora: a busca supõe que os dois
diários são iguais até um ponto e diferentes dali em diante; o evento ilegível
conta como diferente, que erra para o lado de segurar mais.
