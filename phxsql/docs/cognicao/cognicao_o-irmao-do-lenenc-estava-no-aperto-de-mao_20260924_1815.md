# O irmão do pânico do `lenenc` estava no aperto de mão — e o primeiro teste dele caiu pelo motivo errado

**Estado:** PENDENTE

Data da descoberta: 24/09/2026, ~18:15 UTC (frente «fáceis D», pedido 544).

## 1. O que aconteceu

O pedido 544 nomeava dois pânicos do parser do DbLink: o `lenenc` com `0xFE` +
`u64::MAX` no MySQL(R) e a contagem de campos `-1` no PostgreSQL(R). Procurando
o irmão pela PERGUNTA («o outro lado derruba este lado pela forma do pacote?»),
e não pelo nome, apareceu um terceiro no mesmo arquivo: o `cadeia_ate_nulo` do
`dblink/mysql.rs`, usado na saudação e na troca de plugin — **antes da
credencial**, então basta quem responde na porta.

## 2. O que eu concluí primeiro, e estava errado

O primeiro teste da saudação curta cortava a saudação em 15 bytes, falhou, e
parecia a prova do pânico. Não era: a mensagem era `saudacao curta demais`, o
`get(i..i+8)` do sal recusando com erro — o teste falhava por um motivo que não
era o defeito, e passaria a «provar» o conserto por engano.

## 3. O que a medição disse

Cortada em 20 bytes (versão, conexão e os 8 bytes do sal), a saudação chega ao
nome do plugin e o pânico aparece: `range start index 39 out of range for slice
of length 20`. A troca de plugin sem NUL (`[0xFE, 'x']`): `range start index 3
out of range for slice of length 2`. Os dois com o código de antes; com o
conserto (o índice nunca sai do pacote), a saudação curta vale como
`mysql_native_password` e a troca para o plugin `x` recusa com «nao suportado».
No `lenenc`, reposto: 2 pânicos por `attempt to add with overflow` e o campo de
10 bytes num pacote de 4 voltando Ok como `"abc"` — o corte calado que o pedido
não nomeava.

## 4. A regra

Antes de aceitar o vermelho, leia a MENSAGEM dele: teste que cai pelo motivo
errado é o mesmo teste que passa por engano, só que antes.

## 5. Como está guardado hoje

Os testes de soquete em `crates/phxsql-server/src/dblink/mysql.rs` (um servidor
falso só, com roteiro) e em `crates/phxsql-server/src/pg/mod.rs`; as guardas
`dblink-mysql-lenenc-embrulha`, `dblink-pg-contagem-negativa` e
`dblink-mysql-cadeia-alem-do-fim`. O teto de colunas passou a morar em
`dblink::TETO_DE_COLUNAS`, um para os dois dialetos.
