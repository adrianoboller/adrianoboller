# F17–F19 Progress — v0.13

## F17 Checkpoint + Rollback — REVIEW

`phxclaw-checkpoint` now contains a deterministic file checkpoint manager:

- UUIDv7 checkpoint identity;
- SHA-256 per file;
- immutable copied file set;
- verify changed/missing files;
- restore only checkpointed files;
- path confinement;
- excludes `.git`, `target`, `node_modules` and `var`.

Native `cargo check/test` and PostgreSQL E2E remain required before DONE.

## F18 MCP + LSP Runtime — REVIEW

`phxclaw-mcp-lsp-runtime` adds JSON-RPC contracts plus:

- LSP `Content-Length` framing encode/decode;
- MCP JSON-lines encode/decode;
- UUIDv7 correlation metadata.

Actual MCP/LSP server process management, cancellation and E2E remain pending.

## F19 Repo Intelligence / Tree-sitter — REVIEW

`phxclaw-repo-intelligence` adds deterministic repository inventory:

- language detection;
- SHA-256 per source file;
- byte/line counts;
- deterministic importance ranking;
- language distribution.

Tree-sitter AST/symbol graph integration remains pending, so F19 is not DONE.
