# Repo Intelligence v0.63

Pipeline: inventory -> Tree-sitter parse -> symbols -> calls/imports -> resolution -> file graph -> PageRank/hotspots -> evidence store.

Supported AST grammars in source: Rust, Python, JavaScript/JSX, TypeScript/TSX, Go, C, C++, Java. C#/SQL/WLanguage/COBOL/etc remain inventoried but do not receive a fabricated AST until a governed grammar adapter is added.

`ParsedWithErrors` is evidence, not success masking. Files >4 MiB are not parsed. `.git`, `target`, `node_modules`, and `var` are excluded and symlinks are not followed.
