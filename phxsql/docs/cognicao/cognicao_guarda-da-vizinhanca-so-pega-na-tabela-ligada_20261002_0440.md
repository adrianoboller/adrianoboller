# A pergunta «há transação na vizinhança?» só se prova com a transação na tabela LIGADA, não na própria

**Estado:** PENDENTE

## 1. O que aconteceu

Pedido 268: o roteiro do `op_migrar_cifra` pergunta `transacao_na_vizinhanca`
antes de congelar (a lição do 426, copiada do `op_marcar_lgpd`). Escrevi o teste
com a transação aberta na PRÓPRIA `clientes` e a migração recusada com
`EM_TRANSACAO`. A guarda `migracao-da-cifra-sem-pergunta-de-transacao`, que
tira a pergunta do roteiro, deu **NÃO PEGOU: 0/1 cairam** — o teste passava com
o defeito reposto.

## 2. O que eu concluí primeiro, e estava errado

Que o teste da própria tabela exercitava a pergunta. Quem recusava era o portão
5 do `despachar` (`barrado_por_carga`/reserva de transação), que lê o campo
`tabela` do pedido: para a tabela do pedido ele já cobre. A pergunta do
roteiro só tem trabalho a fazer onde o portão não olha — a tabela **ligada pela
chave** (a filha com FK para a mãe), que é o que o 426 mediu.

## 3. O que a medição disse

Antes: `PASSOU COM O DEFEITO REPOSTO: transacao_viva_barra_a_migracao_e_solta_libera`
(0/1). Depois de escrever o teste da vizinhança
(`transacao_na_tabela_ligada_pela_chave_barra_a_migracao`: transação só na
`filha`, migração de `clientes`): **PROVADA, 1/1 cairam** com a pergunta tirada,
e verde com ela.

## 4. A regra

Teste de uma guarda que o portão geral já cobre não prova a guarda: monte o
caso que SÓ ela alcança (aqui, a tabela vizinha por chave) e meça com o
defeito reposto antes de acreditar.

## 5. Como está guardado hoje

Teste `transacao_na_tabela_ligada_pela_chave_barra_a_migracao` e guarda
`migracao-da-cifra-sem-pergunta-de-transacao`. O teste da própria tabela
continua no arquivo, com o docstring dizendo o que ele **não** prova. **Buraco:**
o `op_marcar_lgpd`, `declarar_fk` e `excluir_fk` têm a mesma pergunta e o
catálogo só tem guarda de vizinhança para as duas reescritas do 426
(`migrar_esquema`, `acrescentar_coluna`) — não medi se as três irmãs também
passam com a pergunta tirada.
