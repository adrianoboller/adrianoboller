# PhxClaw Private Plugin Lifecycle — v0.13

PhxClaw remains private-only. v0.13 adds a local/private package lifecycle through `phx`.

## Commands

```text
phx plugins list
phx plugins show <name>
phx plugins install <path-or-.phxplugin>
phx plugins enable <name>
phx plugins disable <name>
phx plugins doctor <name>
phx plugins rollback <name>
phx plugins uninstall <name>
```

## Security gates

Installation is fail-closed and requires:

1. `PACKAGE.json` format `phxclaw-plugin-package-v1`.
2. Complete file inventory with SHA-256 for every package file except `PACKAGE.json` itself.
3. `manifest_sha256` over the complete manifest bytes.
4. `files_digest` over the canonical sorted `path=sha256` inventory.
5. Ed25519 package signature binding UUID, name, version, manifest hash and file inventory digest.
6. Ed25519 plugin artifact signature.
7. Trusted active signer and allowed plugin namespace.
8. UUIDv7 plugin identity.
9. Core API compatibility.
10. No path traversal and no symlinks.
11. Disabled state after install. Enable requires `doctor` first.

The package-level signature was added because signing only the executable artifact does not cryptographically bind permissions, capabilities and sandbox policy. v0.13 therefore signs the manifest hash and full package inventory too.

## Runtime state

The bootstrap CLI stores local state under:

```text
var/plugin-registry/installed.json
var/plugin-store/<plugin-uuid>/<version>/
```

PostgreSQL remains the official persistent state store for production. Migration `0015_private_plugins_checkpoint_repo.sql` prepares the production tables.
