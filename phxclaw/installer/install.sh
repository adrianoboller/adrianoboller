#!/usr/bin/env sh
set -eu
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
PY=python3
command -v "$PY" >/dev/null 2>&1 || { echo 'Python 3 is required.' >&2; exit 2; }
"$PY" "$ROOT/installer/bootstrap.py" install --yes
printf '%s\n' 'PhxClaw bootstrap completed.' 'Next: ./bin/phx core status'
