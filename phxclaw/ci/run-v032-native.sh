#!/usr/bin/env bash
set -Eeuo pipefail
python3 tools/verify_v032.py
python3 tools/test_v032_local.py
cargo fmt --check
if [[ -f Cargo.lock ]]; then cargo check --workspace --locked; cargo test --workspace --locked; cargo clippy --workspace --all-targets --locked -- -D warnings; else echo 'Cargo.lock missing: run cargo generate-lockfile, review and commit it before release'; exit 2; fi
