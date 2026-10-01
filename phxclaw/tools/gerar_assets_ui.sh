#!/usr/bin/env bash
# Gera os JSON que as telas do Command Center leem, de onde o dado nasce -- nenhum numero da
# interface e digitado no HTML:
#   assets/ferramentas.json  <- `phxclaw ferramentas` (a mesma Montagem do agente)
#   assets/absorcao.json     <- docs/absorcao/absorcao.json (gerado pelo gerar_absorcao.py)
# O assets/equipe.json NAO e daqui: sai do `cargo run -p phxclaw-agent --example equipe_json`,
# com teste proprio que reprova o arquivo velho.
#
# Uso: tools/gerar_assets_ui.sh [binario-phxclaw]   (padrao: target/debug/phxclaw)
set -euo pipefail
RAIZ=$(cd "$(dirname "$0")/.." && pwd)
ASSETS="$RAIZ/apps/phxclaw-ui/assets"
BIN=${1:-"$RAIZ/target/debug/phxclaw"}
if [[ ! -x "$BIN" ]]; then
  (cd "$RAIZ" && CARGO_INCREMENTAL=0 cargo build -q -p phxclaw)
fi
# Grava num temporario e so entao troca: um binario que falhe no meio nao deixa a tela
# lendo meio JSON.
TMP=$(mktemp)
trap 'rm -f "$TMP"' EXIT
"$BIN" ferramentas > "$TMP"
python3 -c 'import json,sys; json.load(open(sys.argv[1]))' "$TMP"
cp "$TMP" "$ASSETS/ferramentas.json"
cp "$RAIZ/docs/absorcao/absorcao.json" "$ASSETS/absorcao.json"
python3 - "$ASSETS" <<'EOF'
import json, sys
from pathlib import Path
a = Path(sys.argv[1])
f = json.loads((a / "ferramentas.json").read_text())
b = json.loads((a / "absorcao.json").read_text())
print(f"ferramentas.json: {f['total']} ferramentas")
print(f"absorcao.json: {len(b)} produtos ({', '.join(b)})")
EOF
