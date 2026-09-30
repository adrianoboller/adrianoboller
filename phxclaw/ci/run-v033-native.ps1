$ErrorActionPreference = "Stop"
cargo generate-lockfile
cargo fmt --check
cargo check --workspace --locked
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
python tools/verify_v033.py
python tools/test_v033_local.py
if ($env:PHXCLAW_TEST_DATABASE_URL) { psql $env:PHXCLAW_TEST_DATABASE_URL -v ON_ERROR_STOP=1 -f migrations/0033_continuous_model_arena.sql }
