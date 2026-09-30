$ErrorActionPreference = "Stop"
cargo fmt --check
cargo check --workspace --locked
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
if (-not $env:DATABASE_URL) { throw "DATABASE_URL required" }
if (-not $env:PHXCLAW_RLS_DATABASE_URL) { throw "PHXCLAW_RLS_DATABASE_URL required" }
psql $env:DATABASE_URL -v ON_ERROR_STOP=1 -f migrations/0036_incident_chaos_skill_router.sql
psql $env:PHXCLAW_RLS_DATABASE_URL -v ON_ERROR_STOP=1 -f tests/sql/v036_rls_e2e.sql
python tools/verify_v036.py
python tools/test_v036_local.py
