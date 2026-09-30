# ADR-0063 — Native Repo Intelligence / Tree-sitter

F19 moves from line-count inventory to AST-based repository intelligence. The parser layer is deterministic and read-only. It supports Rust, Python, JavaScript/JSX, TypeScript/TSX, Go, C, C++ and Java with pinned Tree-sitter grammar crates.

Outputs: parse health, symbols, calls, import/include dependencies, exact-match call resolution, file graph PageRank and complexity-weighted hotspots. Unsupported languages remain visible in inventory and are explicitly marked unsupported instead of guessed.

Repository analysis evidence is append-only in PostgreSQL and tenant-isolated by RLS.
