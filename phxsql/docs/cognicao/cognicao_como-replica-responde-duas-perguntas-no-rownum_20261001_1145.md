# A marca `como_replica` responde «quem julga?», e não «de quem é a ordem?»

**Estado:** PENDENTE

## O que aconteceu

Pedido 309 (via (b) do 291): a réplica fiel e o PITR passaram a honrar o
`rownum` que vem na imagem. O precedente era o `carimbar_linha`
(`crates/phxsql-store/src/table.rs`), que já honra o `rowstamp` sob
`como_replica`. Copiar o precedente — honrar o `rownum` também sob
`como_replica` — compilaria e passaria na prova da réplica fiel.

## O que eu concluí primeiro, e estava errado

Que bastava um `if self.como_replica` no `rownum_para`, «no espelho do
carimbo». Só que `como_replica` é aceso por **dois** caminhos: o
`reaplicar_evento_do_proprio_diario` (réplica fiel e PITR, pelo rowid) e os
três `*_replicado` do bidirecional (pela chave). A marca responde «esta escrita
já foi julgada em outro lugar?», e a resposta é sim nos dois. A pergunta do
`rownum` é outra — «de quem é a ordem de digitação?» — e a resposta diverge: na
réplica fiel é do source; no bidirecional é de cada servidor
(`REPLICACAO.md` §12). O carimbo pode seguir a marca porque ele é identidade
de nascimento e viaja igual nos dois modos; o `rownum` não.

## O que a medição disse

Pelo soquete, `tests/rownum-honrado-na-replica.rs`, com source de buraco
fabricado (`1,2,4`): com o honrar ligado só pelo `reaplicar_*`, réplica fiel e
PITR dão `[1,2,4]` e o par bidirecional dá `[1,2,3]`. Com o honrar posto também
no `inserir_replicado` (a cópia ingênua do precedente), o bidirecional passa a
dar `[1,2,4]` — o número do outro servidor dentro do `.reg` daqui. Com o honrar
nunca ligado, réplica fiel e PITR voltam a `[1,2,3]`. As duas reposições estão
no catálogo de guardas e foram provadas (`replica-renumera-o-buraco-do-source`
2/2, `bidirecional-honra-o-rownum-do-outro` 1/1).

## A regra

Antes de pendurar comportamento novo numa marca que já existe, nomeie a
pergunta que a marca responde e confira se é a mesma do comportamento novo em
**todo** caminho que a acende.

## Como está guardado hoje

Campo próprio, `honrar_rownum`, ligado só pelo
`reaplicar_evento_do_proprio_diario`, com o motivo no comentário do campo; as
duas guardas acima travam os dois sentidos. O buraco que fica: a alteração de
uma linha migrada (rownum 0) ainda numera localmente nos dois lados — o
contrato do J limitou o 309 à inclusão.
