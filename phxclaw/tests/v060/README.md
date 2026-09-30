# Test plan

## Executable in this host

- `python scripts/verify_v060.py`

## Native gates still required

- `cargo fmt --check -p phxclaw-provenance-core`
- `cargo check -p phxclaw-provenance-core`
- `cargo test -p phxclaw-provenance-core`
- `cargo clippy -p phxclaw-provenance-core --all-targets -- -D warnings`
- PostgreSQL apply/rollback/E2E for the v0.60 migration
- integration test proving Plugin Registry/Installer refuses QUARANTINE and DENY artifacts
