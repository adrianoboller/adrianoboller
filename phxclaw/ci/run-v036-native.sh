#!/usr/bin/env bash
set -euo pipefail
cargo fmt --check
cargo check --workspace --locked
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
: "${DATABASE_URL:?admin/migration DATABASE_URL required}"
: "${PHXCLAW_RLS_DATABASE_URL:?non-superuser/NOBYPASSRLS URL required}"
psql "$DATABASE_URL" -v ON_ERROR_STOP=1 -f migrations/0036_incident_chaos_skill_router.sql
psql "$PHXCLAW_RLS_DATABASE_URL" -v ON_ERROR_STOP=1 -f tests/sql/v036_rls_e2e.sql
python tools/verify_v036.py
python tools/test_v036_local.py
