#!/usr/bin/env bash
set -euo pipefail
ROOT="${1:-.}"; PLAN="${2:?rollout plan}"; HEALTH="${3:?health evidence}"; OUT="${4:-var/fleet/state.json}"
python3 "$ROOT/tools/verify_v028.py"
python3 "$ROOT/tools/evaluate_rollout.py" "$ROOT" --rollout-plan "$PLAN" --health-evidence "$HEALTH" --out "$OUT"
