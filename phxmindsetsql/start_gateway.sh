#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")"
export PHX_WEB_ROOT="${PHX_WEB_ROOT:-$PWD}"
export PHX_PROFILES_FILE="${PHX_PROFILES_FILE:-$PWD/gateway/profiles.json}"
echo "PhxMindSetSQL v0.7 — http://127.0.0.1:8787"
echo "Profiles: $PHX_PROFILES_FILE"
cargo run --release -p phx-sql-gateway
