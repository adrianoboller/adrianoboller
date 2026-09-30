#!/usr/bin/env bash
set -euo pipefail
python3 tools/verify_v031.py
python3 tools/test_v031_local.py
cargo generate-lockfile
cargo fmt --check
cargo check --workspace --locked
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
# Provider E2E requires explicitly configured test credentials and is never auto-enabled.
