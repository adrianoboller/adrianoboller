#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DEST="$ROOT/private/vendor/openclaw-rs/upstream"
REF="${PHOENIX_UPSTREAM_REF:-main}"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

git clone --depth 1 --branch "$REF" https://github.com/neul-labs/openclaw-rs.git "$TMP/openclaw-rs"
[ -f "$TMP/openclaw-rs/LICENSE" ] || { echo "REFUSED: upstream LICENSE missing" >&2; exit 42; }
grep -q "MIT License" "$TMP/openclaw-rs/LICENSE" || { echo "REFUSED: expected MIT license not found" >&2; exit 43; }
rm -rf "$DEST"
mkdir -p "$(dirname "$DEST")"
cp -a "$TMP/openclaw-rs" "$DEST"
COMMIT="$(git -C "$DEST" rev-parse HEAD)"
python3 - "$DEST" "$COMMIT" <<'PY'
import hashlib,json,sys
from pathlib import Path
root=Path(sys.argv[1]); commit=sys.argv[2]
lock=root.parent/'UPSTREAM.lock.json'
lock.write_text(json.dumps({
  'source':'git_clone','upstream_repository':'https://github.com/neul-labs/openclaw-rs',
  'commit':commit,'license':'MIT','license_file_present':True,
  'vendored_path':'private/vendor/openclaw-rs/upstream'
},indent=2)+'\n')
PY
echo "VENDORED openclaw-rs commit=$COMMIT"
