#!/usr/bin/env bash
set -euo pipefail
cargo fmt --check
cargo check --workspace --locked
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
: "${DATABASE_URL:?DATABASE_URL admin/migration URL required}"
: "${PHXCLAW_RLS_DATABASE_URL:?separate NOSUPERUSER NOBYPASSRLS URL required}"
psql "$DATABASE_URL" -v ON_ERROR_STOP=1 -f migrations/0038_autonomous_engineering_swarm.sql
psql "$PHXCLAW_RLS_DATABASE_URL" -v ON_ERROR_STOP=1 -f tests/sql/v038_rls_e2e.sql
