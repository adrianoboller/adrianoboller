#!/usr/bin/env bash
set -euo pipefail
: "${PHXCLAW_GA_VERSION:?}"
: "${PHXCLAW_GA_SEQUENCE:?}"
: "${PHXCLAW_GA_BASE_URL:?}"
: "${PHXCLAW_GA_PREVIOUS_SEQUENCE:?}"
python3 tools/build_ga_release.py . \
  --rc "${PHXCLAW_RC_ARCHIVE:?}" \
  --multi-platform "${PHXCLAW_MULTI_PLATFORM:?}" \
  --compatibility-matrix "${PHXCLAW_COMPAT_MATRIX:?}" \
  --version "$PHXCLAW_GA_VERSION" --sequence "$PHXCLAW_GA_SEQUENCE" --previous-sequence "$PHXCLAW_GA_PREVIOUS_SEQUENCE" \
  --base-url "$PHXCLAW_GA_BASE_URL" --output "dist/ga/$PHXCLAW_GA_VERSION" "$@"
