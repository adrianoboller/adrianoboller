#!/usr/bin/env sh
set -eu
PORT="${1:-8080}"
echo "PhxMindSetSQL: http://localhost:${PORT}"
python3 -m http.server "$PORT"
