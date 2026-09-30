#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"; cd "$ROOT"
python3 tools/verify_v035.py
python3 tools/test_v035_local.py
python3 tools/check_migration_quoting.py
python3 tools/check_package_v035.py
command -v cargo >/dev/null
command -v rustc >/dev/null
cargo generate-lockfile
cargo fmt --check
cargo check --workspace --locked
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
if [[ -n "${DATABASE_URL:-}" ]]; then
  command -v psql >/dev/null
  psql "$DATABASE_URL" -v ON_ERROR_STOP=1 -f migrations/0035_ai_sre_autopilot.sql
fi
if [[ "${PHXCLAW_RUN_V035_RLS_E2E:-0}" == "1" ]]; then
  : "${PHXCLAW_RLS_DATABASE_URL:?PHXCLAW_RLS_DATABASE_URL is required for RLS E2E}"
  command -v psql >/dev/null
  ROLE_OK="$(psql "$PHXCLAW_RLS_DATABASE_URL" -Atv ON_ERROR_STOP=1 -c "SELECT CASE WHEN rolsuper OR rolbypassrls THEN 'unsafe' ELSE 'ok' END FROM pg_roles WHERE rolname=current_user")"
  [[ "$ROLE_OK" == "ok" ]] || { echo "RLS E2E requires NOSUPERUSER + NOBYPASSRLS role" >&2; exit 1; }
  psql "$PHXCLAW_RLS_DATABASE_URL" -v ON_ERROR_STOP=1 -f tests/sql/v035_rls_e2e.sql
fi
