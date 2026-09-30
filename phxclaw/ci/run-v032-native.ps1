$ErrorActionPreference = "Stop"
python tools/verify_v032.py
python tools/test_v032_local.py
cargo fmt --check
if (Test-Path Cargo.lock) { cargo check --workspace --locked; cargo test --workspace --locked; cargo clippy --workspace --all-targets --locked -- -D warnings } else { Write-Error "Cargo.lock missing: run cargo generate-lockfile, review and commit it before release" }
