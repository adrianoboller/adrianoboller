#!/usr/bin/env bash
set -euo pipefail
python tools/verify_v029.py
if ! command -v cargo >/dev/null; then echo 'cargo unavailable: native gate not passed' >&2; exit 2; fi
cargo generate-lockfile
cargo fmt --check
cargo check --workspace --locked
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
