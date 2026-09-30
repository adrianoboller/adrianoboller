#!/usr/bin/env bash
set -euo pipefail
cargo generate-lockfile
cargo fmt --check
cargo check --workspace --locked
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
python3 tools/verify_v033.py
python3 tools/test_v033_local.py
if command -v psql >/dev/null 2>&1 && [[ -n "${PHXCLAW_TEST_DATABASE_URL:-}" ]]; then
  psql "$PHXCLAW_TEST_DATABASE_URL" -v ON_ERROR_STOP=1 -f migrations/0033_continuous_model_arena.sql
fi
