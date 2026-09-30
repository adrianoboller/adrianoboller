#!/usr/bin/env bash
set -euo pipefail

TOOLCHAIN="${1:-1.98.1}"
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUT="$ROOT/knowledge/offline/rust"

if ! command -v rustup >/dev/null 2>&1; then
  echo "ERROR: rustup not found. Install from https://rustup.rs/ on a connected host." >&2
  exit 2
fi
if ! command -v python3 >/dev/null 2>&1; then
  echo "ERROR: python3 is required to build the offline search index." >&2
  exit 2
fi

rustup toolchain install "$TOOLCHAIN" --profile default
rustup component add rust-docs --toolchain "$TOOLCHAIN"
SYSROOT="$(rustc +"$TOOLCHAIN" --print sysroot)"
DOCSRC="$SYSROOT/share/doc/rust/html"

if [[ ! -d "$DOCSRC" ]]; then
  echo "ERROR: rust-docs component did not produce $DOCSRC" >&2
  exit 3
fi

rm -rf "$OUT/html"
mkdir -p "$OUT/html" "$OUT/index"
cp -a "$DOCSRC/." "$OUT/html/"
python3 "$ROOT/scripts/index_offline_docs.py" \
  --root "$OUT/html" \
  --out "$OUT/index/documents.jsonl"

VERSION="$(rustc +"$TOOLCHAIN" --version)"
STAMP="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
cat > "$OUT/snapshot.json" <<JSON
{
  "toolchain": "$TOOLCHAIN",
  "rustc": "$VERSION",
  "synced_at": "$STAMP",
  "source": "rustup component rust-docs",
  "local_root": "knowledge/offline/rust/html"
}
JSON

echo "Rust offline docs synchronized at: $OUT"
