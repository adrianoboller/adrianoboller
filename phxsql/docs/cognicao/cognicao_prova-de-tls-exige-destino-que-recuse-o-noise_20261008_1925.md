# Prova de que o cliente falou TLS exige um destino que RECUSE o Noise

**Estado:** PENDENTE

## O que aconteceu

Pedido 572, T6b-2: o `pino_tls` entrou nos iniciadores (replica, cluster,
sonda, DbLink `phxsql`, console, `Remoto` da interface e o driver ODBC). A
primeira versao do teste do DbLink
(`crates/phxsql-server/src/servidor/testes_dblink_cifra.rs::a_ligacao_phxsql_fala_tls_pelo_pino_tls_e_o_salvar_o_herda`)
subia o destino com `"tls": true` e `cifra_fio.exigir`, e conferia que a
ligacao com `pino_tls` abria.

## O que eu concluí primeiro, e estava errado

Que «o destino exige cifra» bastava para provar que a ligacao foi por TLS. Nao
basta: o pino TLS faz `Definicao::cifra()` valer `true`, entao uma ligacao que
IGNORASSE o `pino_tls` cairia no Noise sem pino -- e o Noise sem pino passa no
`exigir` do mesmo jeito. O teste passaria com o defeito reposto, pelo motivo
errado.

## O que a medição disse

Com o destino em `cifra_fio.ligada: false` (o Noise recusado) e `exigir: true`,
so o TLS atravessa. O defeito reposto (`pino_tls()` filtrado para `None` em
`dblink/phx.rs`) passou a derrubar o teste. O mesmo cuidado ja estava no
teste do cluster (`tests/fio-tls-entre-phxsql.rs`), por outro caminho: ali a
`cluster.cifra` e `false`, entao sem o TLS o pulso sai em CLARO e o `exigir`
o recusa.

E o vetor do `tls-exporter` (RFC 9266): nao ha traco oficial do valor -- a RFC
8448 para no `exp master`. O vetor usado e o `-keymatexport
EXPORTER-Channel-Binding -keymatexportlen 32` do `openssl s_client`/`s_server`
na MESMA conexao, nos dois sentidos, com ChaCha20/X25519 e com AES-128-GCM
depois de HRR. Trocar `Hash(contexto)` por `contexto` no `tls13::exportar`
derruba o do servidor; derivar o `exp master` da transcricao errada derruba o
do cliente.

## A regra

Para provar que uma conexao usou o caminho NOVO, o outro lado tem de RECUSAR o
caminho velho -- aceitar os dois prova so que alguma cifra houve.

## Como está guardado hoje

Nos tres testes de soquete do T6b-2: DbLink (destino sem Noise), cluster
(`cluster.cifra: false` + `exigir`) e `Remoto` (`cifra: false` + destino que
exige). O console e o `Cliente` conferem pelo lado cliente (`Cliente::tls()`),
e o pino errado recusa nos dois. Nao ha guarda no catalogo para isto ainda.
