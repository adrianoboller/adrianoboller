# ADR-0001 — Isolamento Parser ↔ Renderer

**Status:** aceito e obrigatório  
**Versão:** v0.4.0

## Decisão

O parser SQL não conhece interface ou renderização. Renderers não fazem parsing SQL e não contêm regras de dialeto.

Todos os parsers convertem sua entrada para um `UnifiedSqlModel` neutro. Todas as visualizações consomem somente esse modelo e projeções derivadas dele.

## Motivos

- adicionar MySQL/SQLite/SQL Server sem duplicar gráficos;
- testar parsing independentemente de UI;
- trocar engine gráfica sem tocar no parser;
- permitir Rust/WASM, parser nativo ou metadata connector usando o mesmo contrato;
- impedir acoplamento progressivo entre regex/AST e componentes visuais.

## Consequências

### Positivas

- plugins de dialeto independentes;
- renderers reutilizáveis;
- testes menores e determinísticos;
- possibilidade de conectar banco vivo e produzir o mesmo modelo;
- evolução do parser PostgreSQL sem regressão gráfica deliberada.

### Custos

- existe uma etapa explícita de normalização;
- recursos exclusivos de um dialeto devem ser representados por extensões/capabilities sem contaminar o contrato base;
- mudanças incompatíveis no modelo exigem versionamento do contrato.

## Enforcement

`npm test` executa `scripts/architecture-check.mjs` e falha quando:

- parsers usam DOM/Canvas/Sigma/ELK;
- renderer importa parser;
- `app.js` volta a conter regex/rotina de parsing SQL;
- crates Rust violam a matriz de dependências.
