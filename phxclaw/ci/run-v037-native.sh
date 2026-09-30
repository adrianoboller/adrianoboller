#!/usr/bin/env bash
set -euo pipefail
: "${DATABASE_URL:?admin/migration DATABASE_URL required}"
: "${PHXCLAW_RLS_DATABASE_URL:?non-superuser RLS connection required}"
cargo generate-lockfile
cargo fmt --check
cargo check --workspace --locked
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
psql "$DATABASE_URL" -v ON_ERROR_STOP=1 -f migrations/0037_autonomous_engineering_workflow.sql
psql "$PHXCLAW_RLS_DATABASE_URL" -v ON_ERROR_STOP=1 -f tests/sql/v037_rls_e2e.sql
python3 tools/verify_v037.py .
python3 tools/test_v037_local.py .
