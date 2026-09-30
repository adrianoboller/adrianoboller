# PhoenixClaw v0.30 — Third-Party Notices Delta

No third-party source tree is vendored by this overlay. PhoenixClaw-authored adapters implement interoperability contracts based on public APIs/formats.

## Obsidian
- `obsidianmd/obsidian-api` — MIT; API/type contract reference. Reviewed package metadata: `obsidian` 1.13.2.
- `obsidianmd/jsoncanvas` — MIT; open JSON Canvas interchange format.
- `obsidianmd/obsidian-sample-plugin` — 0BSD; plugin lifecycle/build reference only.
- `obsidianmd/obsidian-importer` — MIT; external-data-to-Markdown import architecture reviewed, no source copied.
- The Obsidian application itself is not copied, modified, vendored, or linked into PhoenixClaw. Integration uses supported plugin APIs and user-owned vault files.

## Hermes Agent
- `NousResearch/hermes-agent` — MIT (current repository LICENSE and project metadata).
- PhoenixClaw adapts interoperability concepts: staged `SKILL.md` import, persistent-memory observations, MCP references, scheduled-task translation, isolated subagent translation, and read-only-first GitHub policy.
- No Hermes source tree is vendored.

## Ollama
- `ollama/ollama` — MIT.
- PhoenixClaw implements provider contracts for local Ollama API, OpenAI-compatible and Anthropic-compatible endpoints. No Ollama source tree or model weights are redistributed.
- Model weights remain subject to each model's own license; PhoenixClaw never treats the Ollama application license as the model license.
