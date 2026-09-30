$ErrorActionPreference = "Stop"
python tools/verify_v030.py
python tools/test_v030_local.py
cargo fmt --check
cargo check --workspace --locked
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
