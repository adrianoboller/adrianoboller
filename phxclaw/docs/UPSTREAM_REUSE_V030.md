# Upstream Reuse Record — PhxClaw v0.30

Reviewed on 2026-09-28.

| Upstream | Public reference | Observed license | PhxClaw use |
|---|---|---|---|
| Obsidian API | https://github.com/obsidianmd/obsidian-api | MIT | Plugin API/type contract reference; reviewed npm metadata 1.13.2 |
| Obsidian JSON Canvas | https://github.com/obsidianmd/jsoncanvas | MIT | `.canvas` interchange for F25 knowledge/evidence visualization |
| Obsidian Sample Plugin | https://github.com/obsidianmd/obsidian-sample-plugin | 0BSD | Lifecycle/build conventions reference only |
| Obsidian Importer | https://github.com/obsidianmd/obsidian-importer | MIT | Import-pipeline architecture reference; no converters copied |
| Hermes Agent | https://github.com/NousResearch/hermes-agent | MIT | Skills/memory/MCP/cron/subagent/GitHub interoperability; no source vendored |
| Ollama | https://github.com/ollama/ollama | MIT | Local/cloud HTTP provider contract, OpenAI/Anthropic compatibility; no source/weights vendored |

## Boundaries
- Obsidian application code is outside this reuse set; the companion plugin uses documented APIs and a file-drop exchange directory.
- Imported Obsidian/Hermes content starts as `unverified_context` unless Phoenix evidence proves otherwise.
- Hermes skills enter F24 as candidates; no import bypasses Promotion Gate.
- GitHub writes are approval-gated; reads are default.
- Ollama local loopback HTTP is allowed; remote origins are deny-by-default and HTTPS+allowlist only when enabled.
- Model licensing remains independent and must be inspected before distribution/use.
