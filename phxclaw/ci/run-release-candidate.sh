#!/usr/bin/env bash
set -Eeuo pipefail
ROOT="${1:-.}"; VERSION="${2:?candidate version required}"; QDIR="${3:-reports/release-qualification/release}"; ARTIFACT="${4:-target/release/phxclaw}"
python3 "$ROOT/tools/build_release_candidate.py" "$ROOT" --qualification-dir "$QDIR" --candidate-version "$VERSION" --artifact "$ARTIFACT" --output "$ROOT/dist/rc/$VERSION" --public-rc "${@:5}"
