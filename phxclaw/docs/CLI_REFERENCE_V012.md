# PhxClaw CLI — v0.12

Entry points:

- Linux/macOS/WSL: `./bin/phx --help`
- Windows: `bin\phx.cmd --help`
- Rust bootstrap app: `apps/phxclaw-cli`
- Mission binary: `apps/phxclaw-mission-cli`

## Commands

```text
phx version
phx status
phx doctor
phx paths
phx privacy
phx sprints list
phx sprints show F17
phx plugins list
phx plugins show openai-platform
phx agents count
phx agents list --limit 20
phx capabilities [--filter rust]
phx mission validate examples/missions/rust-validation.mission.json
phx mission run examples/missions/rust-validation.mission.json
```

`mission run` is fail-closed until the Rust release binary exists.
