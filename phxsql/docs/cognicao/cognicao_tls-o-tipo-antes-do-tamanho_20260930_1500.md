# TLS: confira o tipo do registro ANTES do tamanho

**Estado:** FRUTÍFERO
**Evidência:** `crates/phxsql-core/src/tls.rs::http_em_claro_recebe_o_alerta_na_hora` e `crates/phxsql-server/tests/tls-das-portas-http.rs::a_porta_tls_nao_responde_em_claro` — com a conferência do cabeçalho tirada, os dois ficam vermelhos (mutante `sem-conferir-cabecalho`, 30/09/2026).

## O que aconteceu

Pedido 572, T4b. A prova «a porta TLS não responde em claro» mandava
`GET /saude HTTP/1.1` cru numa porta com `"tls": true` e esperava um alerta.
Veio **nada**, depois de 10 s.

## O que eu concluí primeiro, e estava errado

Que o servidor já recusava: o teto do registro (2^14 + 256) existia, e um
registro que não é TLS «tem de» estourar alguma conferência. O teste do
T4a tinha até um caso de `ClientHello` gigante, verde.

Errado: os cinco primeiros bytes de `GET /` são lidos como cabeçalho, e o
tamanho sai de `T` e espaço — `0x202f`, **8.239 bytes**, abaixo do teto. O
servidor fica esperando esses bytes até o prazo de leitura, e o cliente não
recebe resposta nenhuma. O teto protegia a memória; não protegia o tempo.

## O que a medição disse

| caso | antes | depois |
|---|---|---|
| `GET /saude` cru na porta TLS | 0 bytes em 10 s | alerta `unexpected_message` na hora |

## A regra

Num protocolo com cabeçalho de tamanho, confira **tudo o que se pode
conferir no cabeçalho** (tipo, versão) antes de acreditar no tamanho. O teto
de tamanho segura a memória; só a conferência do tipo segura o tempo de quem
fala o protocolo errado.

## Como está guardado hoje

Os dois testes acima, com o defeito reposto medido vermelho.
