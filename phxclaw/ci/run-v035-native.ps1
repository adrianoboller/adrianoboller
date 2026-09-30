$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent $PSScriptRoot
Set-Location $Root
python tools/verify_v035.py
python tools/test_v035_local.py
python tools/check_migration_quoting.py
python tools/check_package_v035.py
if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) { throw "cargo unavailable" }
if (-not (Get-Command rustc -ErrorAction SilentlyContinue)) { throw "rustc unavailable" }
cargo generate-lockfile
cargo fmt --check
cargo check --workspace --locked
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
if ($env:DATABASE_URL) {
  if (-not (Get-Command psql -ErrorAction SilentlyContinue)) { throw "psql unavailable" }
  psql $env:DATABASE_URL -v ON_ERROR_STOP=1 -f migrations/0035_ai_sre_autopilot.sql
}
if ($env:PHXCLAW_RUN_V035_RLS_E2E -eq "1") {
  if (-not $env:PHXCLAW_RLS_DATABASE_URL) { throw "PHXCLAW_RLS_DATABASE_URL is required for RLS E2E" }
  if (-not (Get-Command psql -ErrorAction SilentlyContinue)) { throw "psql unavailable" }
  $roleOk = (psql $env:PHXCLAW_RLS_DATABASE_URL -Atv ON_ERROR_STOP=1 -c "SELECT CASE WHEN rolsuper OR rolbypassrls THEN 'unsafe' ELSE 'ok' END FROM pg_roles WHERE rolname=current_user").Trim()
  if ($roleOk -ne "ok") { throw "RLS E2E requires NOSUPERUSER + NOBYPASSRLS role" }
  psql $env:PHXCLAW_RLS_DATABASE_URL -v ON_ERROR_STOP=1 -f tests/sql/v035_rls_e2e.sql
}
