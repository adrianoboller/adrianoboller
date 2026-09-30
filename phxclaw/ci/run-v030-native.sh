#!/usr/bin/env bash
set -euo pipefail
python3 tools/verify_v030.py
python3 tools/test_v030_local.py
cargo fmt --check
cargo check --workspace --locked
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
