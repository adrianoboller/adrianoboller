$ErrorActionPreference = "Stop"
if (-not $env:DATABASE_URL) { throw "DATABASE_URL required" }
if (-not $env:PHXCLAW_RLS_DATABASE_URL) { throw "PHXCLAW_RLS_DATABASE_URL required" }
cargo generate-lockfile
cargo fmt --check
cargo check --workspace --locked
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
psql $env:DATABASE_URL -v ON_ERROR_STOP=1 -f migrations/0039_swarm_consensus_merge_intelligence.sql
psql $env:PHXCLAW_RLS_DATABASE_URL -v ON_ERROR_STOP=1 -f tests/sql/v039_rls_e2e.sql
