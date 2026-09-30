#!/usr/bin/env bash
set -euo pipefail
: "${DATABASE_URL:?admin/migration DATABASE_URL required}"
: "${PHXCLAW_RLS_DATABASE_URL:?separate NOSUPERUSER/NOBYPASSRLS URL required}"
cargo generate-lockfile
cargo fmt --check
cargo check --workspace --locked
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
psql "$DATABASE_URL" -v ON_ERROR_STOP=1 -f migrations/0039_swarm_consensus_merge_intelligence.sql
psql "$PHXCLAW_RLS_DATABASE_URL" -v ON_ERROR_STOP=1 -f tests/sql/v039_rls_e2e.sql
