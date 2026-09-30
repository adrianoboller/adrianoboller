# Validação estática não é execução: 31 PASS e o instalador parava na linha 4306

**Estado:** FRUTÍFERO

**Evidência:** `8e22e6e`; `5205e0d`

## O que aconteceu

O pacote v0.70 declarava «Validação estática SQL: 31 PASS / 0 FAIL» e «fontes: 11 PASS».
Na primeira execução real: 13 crates não compilavam, o `FULL_INSTALL` parava na linha 4306
(`array_to_string` é STABLE em coluna gerada), 21 políticas RLS liam uma GUC que o app
nunca seta, a UNIQUE de `knowledge_nodes` recusava a versão que o próprio motor gera, e o
`claim_ready_token` do BPM falhava **sempre** (parâmetro sem tipo que o rust-postgres
prepara).

## O que eu concluí primeiro, e estava errado

Que «31 PASS» reduzia o risco do SQL a detalhe. Nenhum dos cinco defeitos é visível a um
validador que lê texto: todos dependem do catálogo, do tipo inferido ou da ordem real.

## O que a medição disse

5 defeitos de banco, 38+ erros de compilação, 4 testes vermelhos — todos na primeira
corrida. Depois dos consertos: 16/16 E2E no PostgreSQL 16.13, 54/54 literais SQL do Rust
preparam contra o esquema instalado.

## A regra

«Verificado» só vale para o que rodou. SQL embutido se prova com `PREPARE` contra o
esquema instalado; migração, contra um servidor real.

## Como está guardado hoje

`tests/postgres/run_e2e.sh` e `tools/pg_prepare_sweep.py`, e o gate `postgresql_e2e` do
`tools/release_certification.py`. Buraco: não há CI rodando isso sozinho.
