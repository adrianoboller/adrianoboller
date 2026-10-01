# Validação v0.7

## Resultado executado

`npm test`: **PASS**.

Cobertura executada:

- syntax check JavaScript;
- architecture-check Parser ↔ Model ↔ Projection ↔ Renderer;
- baseline PhxClaw;
- detector de 4 dialetos;
- adapters de 4 dialetos;
- paridade de projeção;
- recursos avançados;
- schema diff;
- renderer isolation;
- contrato Live DB;
- arquitetura de introspecção;
- validação estática do workspace Rust.

## Baseline

- tabelas: 294;
- colunas: 2.837;
- FKs: 254;
- CHECKs: 669;
- generated columns: 2;
- extensions: 1 (`pgcrypto`).

## Recursos avançados por fixtures

- PostgreSQL: CHECK, generated, sequence, materialized view, partition, extension;
- MySQL: CHECK, generated, partition, event;
- SQLite: CHECK, generated;
- SQL Server: CHECK, computed, sequence, synonym.

## Rust

Foi criado `crates/phx-sql-core/tests/advanced_features.rs` com fixtures equivalentes. Nesta máquina não há `cargo/rustc`, portanto esses testes não foram executados; a validação Rust feita aqui é estrutural/estática.

## SQL baseline

SHA-256 esperado e preservado:

`0de68d3710d70ceb531d573c5b3e1306a527c26e9d384231d67a0f7bb27fee29`
