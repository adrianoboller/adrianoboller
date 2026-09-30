# PhxClaw CLI — v0.13

```text
phx version
phx status
phx doctor
phx paths
phx privacy
phx sprints list
phx sprints show F17
phx plugins list
phx plugins show <name>
phx plugins install <package-dir-or-.phxplugin>
phx plugins enable <name>
phx plugins disable <name>
phx plugins doctor <name> [--static-only]
phx plugins rollback <name>
phx plugins uninstall <name>
phx agents count
phx agents list --limit 20
phx capabilities --filter rust
phx mission validate <file.json>
phx mission run <file.json>
phx repo scan [path] --limit 20
```

`mission run` remains fail-closed until the native Rust mission binary is built.

Plugin installation is private-only and validates the full signed package: file inventory, manifest hash, Ed25519 package signature, artifact signature, UUIDv7 and Core API compatibility.
