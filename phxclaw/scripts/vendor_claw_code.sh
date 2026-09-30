#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DEST="${PHXCLAW_CLAW_VENDOR_DIR:-$ROOT/private/vendor/claw-code/upstream}"
REF="${CLAW_CODE_REF:-main}"
REPO="https://github.com/ultraworkers/claw-code.git"

command -v git >/dev/null 2>&1 || { echo "git is required" >&2; exit 2; }
rm -rf "$DEST.tmp"
git clone --filter=blob:none --no-checkout "$REPO" "$DEST.tmp"
git -C "$DEST.tmp" checkout "$REF"
rm -rf "$DEST"
mv "$DEST.tmp" "$DEST"
COMMIT="$(git -C "$DEST" rev-parse HEAD)"
python3 - "$ROOT" "$DEST" "$REF" "$COMMIT" <<'PY'
import json,sys,datetime,pathlib
root=pathlib.Path(sys.argv[1]); dest=pathlib.Path(sys.argv[2]); ref=sys.argv[3]; commit=sys.argv[4]
out=root/'private/vendor/claw-code/UPSTREAM.lock.json'
out.write_text(json.dumps({
  'repository':'https://github.com/ultraworkers/claw-code',
  'requested_ref':ref,
  'commit':commit,
  'fetched_at':datetime.datetime.now(datetime.timezone.utc).isoformat(),
  'path':str(dest.relative_to(root)),
  'license':'MIT',
  'canonical_source':'rust/'
},indent=2)+"\n")
PY
cp "$DEST/LICENSE" "$ROOT/private/vendor/claw-code/LICENSE.upstream" 2>/dev/null || true
printf 'Claw Code vendored at %s\ncommit=%s\n' "$DEST" "$COMMIT"
