$ErrorActionPreference = "Stop"
python tools/verify_v029.py
if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) { throw "cargo unavailable: native gate not passed" }
cargo generate-lockfile
cargo fmt --check
cargo check --workspace --locked
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
