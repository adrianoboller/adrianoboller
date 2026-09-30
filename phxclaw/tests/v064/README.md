# v0.64 MCP/LSP process fixtures

`mcp_stdio_fixture.py` and `lsp_stdio_fixture.py` are real child processes used to prove wire framing/lifecycle on this host. They are test fixtures, not production fallbacks.

`tools/v064_process_smoke.py` spawns both and executes initialize + invocation + shutdown flows.

The production Rust runtime must still pass `cargo test` and E2E against independent real MCP/LSP servers before the release gate becomes green.
