# Validação — PhxMindSetSQL v0.4.0

## Executado neste ambiente

### JavaScript

`npm test` passou integralmente:

- sintaxe dos módulos: OK;
- arquitetura parser/renderer: OK;
- baseline: OK;
- detector de dialetos: OK.

### Baseline

- 294 tabelas
- 2.837 colunas
- 254 FKs
- 55 migrations
- 46 funções
- 58 triggers
- 76 índices
- 68 policies
- 5 schemas

### Detector

Casos sintéticos confirmados:

- PostgreSQL: OK
- MySQL: OK
- SQLite: OK
- SQL Server: OK

### Projeção

O mesmo baseline gerou:

- 294 graph nodes de tabela
- 254 graph edges de FK

## Arquitetura

Verificado automaticamente:

- parser JS não usa DOM/UI;
- parser JS não importa renderer;
- renderer avançado não importa parsers;
- `app.js` não contém parser SQL;
- model crate não depende de parser/projection/UI;
- PostgreSQL crate não depende de renderer/projection;
- projection crate não depende de parser/UI.

## Rust

O ambiente atual não possui `cargo`, `rustc` ou `wasm-pack`. Por isso o workspace Rust v0.4 não pôde ser compilado aqui. Os arquivos foram reestruturados para crates independentes e os scripts de build foram preservados para execução em ambiente Rust.

## Browser

O Chromium do ambiente está sujeito a política administrativa que bloqueia navegação para `localhost` e `file://`, portanto a regressão visual automatizada não pôde ser repetida nesta revisão. A validação JS/modelo foi executada sem browser.
