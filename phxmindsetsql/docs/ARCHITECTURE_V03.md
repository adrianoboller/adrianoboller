# Arquitetura — PhxMindSetSQL v0.3

## Princípio

**Uma informação, quatro projeções.** Nenhuma visualização possui seu próprio modelo de dados. Todas consomem o mesmo `Unified SQL Model`.

## Pipeline

1. Usuário abre `.sql`.
2. `src/sql-worker.js` recebe o conteúdo.
3. Worker tenta inicializar `pkg/phx_sql_core.js` (Rust/WASM).
4. Se o WASM não estiver presente, executa `src/parser-fallback.js` no próprio worker.
5. O modelo volta para a main thread.
6. Os quatro renderers consomem o mesmo modelo.

## Camada PRO

`src/enhancer.js` faz progressive enhancement:

- Sigma.js/Graphology → Obsidian WebGL;
- ForceAtlas2 → espacialização do grafo;
- ELK → layout do DER;
- se algum import falhar, a aplicação mantém os renderers locais.

## Contrato do modelo

Campos principais:

- `source`
- `stats`
- `migrations[]`
- `tables[]`
- `relationships[]`
- `objects[]`

Cada tabela contém `columns`, `primary_key`, `foreign_keys`, `ddl`, `migration` e `degree`.

## Isolamento

O parser não conhece componentes de interface. Os renderers não fazem parsing de SQL. Isso permite trocar o parser PostgreSQL, adicionar MySQL/SQLite/SQL Server e manter os mesmos gráficos.
