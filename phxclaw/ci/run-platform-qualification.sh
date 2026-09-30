#!/usr/bin/env bash
set -euo pipefail
ROOT="${1:-.}"; RC="${2:?RC zip required}"; ART="${3:?artifact required}"; OUT="${4:?evidence dir required}"; CAND="${5:?candidate version required}"; PLATFORM="${6:?platform required}"; ARCH="${7:?architecture required}"; shift 7
python3 "$ROOT/tools/run_platform_gates.py" "$ROOT" --rc "$RC" --artifact "$ART" --evidence-dir "$OUT" --candidate-version "$CAND" --platform "$PLATFORM" --architecture "$ARCH" "$@"
python3 "$ROOT/tools/create_platform_evidence.py" "$ROOT" --artifact "$ART" --evidence-dir "$OUT" --output "$OUT/platform-evidence.json"
