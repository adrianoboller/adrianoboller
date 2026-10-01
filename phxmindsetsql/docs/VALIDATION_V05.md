# Validação — v0.5.0

## Baseline
- 294 tabelas
- 2.837 colunas
- 254 FKs
- 55 migrations
- 46 funções
- 58 triggers
- 76 índices
- 68 policies

## Testes Web executáveis

`npm test` cobre sintaxe JS, arquitetura, baseline, detector, parsers, paridade e imutabilidade dos renderers.

Os fixtures `tests/fixtures/equivalent.*.sql` representam o mesmo schema lógico nos quatro dialetos e devem produzir 3 tabelas, 8 campos, 2 FKs e a mesma `GraphProjection`.

## Rust

Os adapters Rust e testes de paridade estão no workspace. Não foi possível executar `cargo test` neste ambiente porque `cargo/rustc` não estão instalados.
