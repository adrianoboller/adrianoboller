# Quem selou é quem diz que selou — o receptor não deduz pelo próprio cofre

**Estado:** PENDENTE

- **Quando:** 2026-10-01, 02:00
- **Onde:** `crates/phxsql-store/src/reg.rs` (`abrir_externo`),
  `crates/phxsql-store/src/table.rs` (`decodificar_com_externos`,
  `imagem_para_o_fio`), `crates/phxsql-server/src/servidor.rs` (`op_replicar`)
- **Pedido:** 344

## O que aconteceu

A coluna externa marcada (`Bin`/`Memo`) viajava na imagem de replicação com
os bytes como estão no `.memo`/`.bin`: selados pela chave do `.reg` da origem.
A réplica abria com `abrir_externo`, que pergunta ao arquivo **daqui** se ele
sela. Medido pelo soquete, origem `phxsqld` com cofre: a réplica sem cofre
gravou **68 bytes** (`[nonce 24][cifrado 28][etiqueta 16]`) no lugar do anexo
de 28 e respondeu `ok`; a réplica com a **mesma senha** parou com «a etiqueta
não confere — ou o dado foi alterado».

## O que eu concluí primeiro, e estava errado

O inventário apontava a causa no `||` de `abrir_externo` (tabela sem cofre e
coluna não marcada no mesmo `return`), e a primeira ideia foi trocar o `||`
por uma recusa quando a tabela não tem cofre. Não fecha: o mesmo
`abrir_externo` serve a leitura local de uma tabela sem cofre, onde devolver
os bytes **é** o certo — e a imagem de um diário de tabela sem cofre também
chega lá com bytes claros. Pelo estado do receptor não há como separar os dois
casos. A pergunta «isto está selado?» só tem resposta em quem selou.

## O que a medição disse

- `tests/coluna-externa-marcada-na-replica.rs`, defeito reposto: 2/2 caem
  (68 bytes de lixo; `SP000010 ... o dado foi alterado`). Com o conserto: 3/3.
- A causa do 344-2 não era nó nem caminho na AAD (a AAD do externo é só o
  índice da coluna): é o **sal por arquivo** — a mesma senha deriva outra chave.

## A regra

Quando um dado atravessa de um arquivo para outro, o **produtor** marca o
estado (selado ou aberto) no próprio dado; o consumidor nunca o deduz pelo
estado dele.

## Como está guardado hoje

Bit `EXTERNO_SELADO` (0x8000) na coluna do externo da imagem
(`docs/FORMATO.md`); o `replicar`, o PITR e a FFI abrem pelo
`imagem_para_o_fio`. Guardas `externo-selado-gravado-como-anexo` e
`replicar-manda-o-externo-selado`. O buraco que fica: o `op_aplicar`, que
recebe imagem de cliente; se vier selada de um diário lido direto, recusa
nomeando — não grava, mas também não replica.
