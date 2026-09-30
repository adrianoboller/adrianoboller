#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DEST="$ROOT/private/vendor/rustclaw/upstream"
REF="${PHOENIX_UPSTREAM_REF:-main}"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

git clone --depth 1 --branch "$REF" https://github.com/Adaimade/RustClaw.git "$TMP/RustClaw"
LICENSE_FILE=""
for candidate in LICENSE LICENSE-MIT LICENSE.txt LICENSE.md; do
  if [ -f "$TMP/RustClaw/$candidate" ]; then LICENSE_FILE="$candidate"; break; fi
done
if [ -z "$LICENSE_FILE" ]; then
  echo "REFUSED: RustClaw clone has no license text; keep source quarantined" >&2
  exit 42
fi
if ! grep -qi "MIT License" "$TMP/RustClaw/$LICENSE_FILE"; then
  echo "REFUSED: RustClaw license is not verified MIT" >&2
  exit 43
fi
rm -rf "$DEST"
mkdir -p "$(dirname "$DEST")"
cp -a "$TMP/RustClaw" "$DEST"
COMMIT="$(git -C "$DEST" rev-parse HEAD)"
python3 - "$DEST" "$COMMIT" "$LICENSE_FILE" <<'PY'
import json,sys
from pathlib import Path
root=Path(sys.argv[1]); commit=sys.argv[2]; license_file=sys.argv[3]
status=root.parent/'LICENSE_STATUS.json'
status.write_text(json.dumps({
  'source':'git_clone','upstream_repository':'https://github.com/Adaimade/RustClaw',
  'commit':commit,'license':'MIT','license_file_present_in_archive':True,
  'license_file':license_file,'reuse_state':'verified_mit_source','compiled_or_linked_into_phxclaw':True,
  'vendored_path':'private/vendor/rustclaw/upstream'
},indent=2)+'\n')
PY
echo "VERIFIED RustClaw MIT source commit=$COMMIT; PhxClaw native compatibility layer may reuse MIT portions with notice"
