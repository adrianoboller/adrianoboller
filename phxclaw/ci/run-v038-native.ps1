$ErrorActionPreference = "Stop"
cargo fmt --check
cargo check --workspace --locked
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
if (-not $env:DATABASE_URL) { throw "DATABASE_URL admin/migration URL required" }
if (-not $env:PHXCLAW_RLS_DATABASE_URL) { throw "PHXCLAW_RLS_DATABASE_URL separate NOSUPERUSER NOBYPASSRLS URL required" }
psql $env:DATABASE_URL -v ON_ERROR_STOP=1 -f migrations/0038_autonomous_engineering_swarm.sql
psql $env:PHXCLAW_RLS_DATABASE_URL -v ON_ERROR_STOP=1 -f tests/sql/v038_rls_e2e.sql
