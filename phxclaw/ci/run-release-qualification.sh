#!/usr/bin/env bash
set -euo pipefail
ROOT="${1:-.}"
OUT="${2:-$ROOT/reports/release-qualification/manual}"
python3 "$ROOT/tools/qualify_release.py" "$ROOT" --output "$OUT" "${@:3}"
