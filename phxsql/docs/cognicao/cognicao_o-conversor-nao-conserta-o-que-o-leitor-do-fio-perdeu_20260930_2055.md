# O conversor não conserta o que o leitor do fio já perdeu

**Estado:** PENDENTE

## O que aconteceu

Pedido 584, medido em 30/09/2026 contra o MySQL(R) falso do soquete: um BLOB
com os bytes `cafe` (quatro bytes ASCII) foi puxado pela sincronia do DbLink e
gravado aqui como os **dois** bytes `CA FE`, sem erro nenhum; um BLOB `00 FF 80`
recusou a rodada («o valor recebido, de 7 bytes, não serve ao tipo»). No
PostgreSQL(R) falso, todo `bytea` (`\x00ff80`) recusava.

## O que eu concluí primeiro, e estava errado

Que o defeito era o `hex_para_bytes` no `linha_remota_para_negocio` — trocá-lo
por «bytes da célula» resolveria. Não resolveria: a célula já chega como
`String`, montada por `String::from_utf8_lossy` no leitor do protocolo
(`mysql.rs::ler_linha`, `pg/mod.rs::ler_linha`). O byte `FF` virou `U+FFFD`
(três bytes) antes de qualquer conversor ver a célula — os 3 bytes viraram 7.
Nenhuma conversão depois do leitor devolve o que ele jogou fora.

## O que a medição disse

- `cafe` → gravado `CA FE` (2 bytes) com a leitura por `SELECT *`; `63616665`
  (4 bytes) com a leitura por `HEX(c)`.
- `00 FF 80` → recusa com «7 bytes» (lossy) contra `00ff80` com a leitura por
  hexadecimal.
- Cinco mutantes repostos (upsert, booleano no empurrão, booleano lido, leitura
  sem hexadecimal, uuid pela carga): cada um derruba pelo menos um dos quatro
  testes de soquete de `servidor::testes_dblink_dialeto_da_sincronia`.

## A regra

Quando o dado chega errado, procure **onde ele deixou de ser o dado** antes de
consertar quem o leu por último: se o leitor do fio perde informação, o conserto
é pedir ao servidor de lá uma forma que o leitor não perde (`HEX()`,
`encode(…,'hex')`), não remendar o conversor.

## Como está guardado hoje

Guarda `dblink-puxar-le-blob-cru` no `bancada/guardas/catalogo.py`. O buraco
que fica: o `dblink_consultar` e o `dblink_ler` da tela continuam mostrando o
BLOB pelo leitor com perda — ali é exibição, e não gravação, mas a célula
mostrada não é o dado.
