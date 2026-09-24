#!/usr/bin/env bash
# Semeia uma rede "Farol" (dono + um membro ja no rol, com o farol marcado)
# e exercita o selo/botao na janela de mesa DE VERDADE no Chromium.
#
# Sem netns/TUN: o segundo membro entra no rol A MAO (o mesmo atalho do
# teste `mesa::testes::farol_pela_api_dono_autoriza_membro_consente`), pelo
# exemplo `semear_farol`. So o HTML/JS/HTTP da janela e exercitado de
# verdade -- e e so isso que esta prova promete provar.
#
# Uso: ./rodar.sh [pasta-de-trabalho]
set -euo pipefail
AQUI=$(cd "$(dirname "$0")" && pwd)
RAIZ=$(cd "$AQUI/../.." && pwd)
T=${1:-$(mktemp -d /tmp/phxvpn-prova-farol-tela.XXXX)}
BIN=${PHXVPN_BIN:-$RAIZ/target/debug/phxvpn}
PORTA=${PORTA:-8490}
mkdir -p "$T/dono" "$T/membro" "$T/capturas"

cargo run --quiet --example semear_farol --manifest-path "$RAIZ/Cargo.toml" -- "$T/dono" "$T/membro"

"$BIN" mesa --pasta "$T/dono" --porta "$PORTA" --sem-janela >"$T/mesa.log" 2>&1 &
PID=$!
trap 'kill -9 $PID 2>/dev/null || true' EXIT
for _ in $(seq 50); do grep -q "janela em" "$T/mesa.log" && break; sleep 0.1; done
FICHA=$(grep -o 'f=[0-9a-f]*' "$T/mesa.log" | head -1 | cut -d= -f2)
URL="http://127.0.0.1:$PORTA/#f=$FICHA"

URL="$URL" SAIDA="${SAIDA:-$T/capturas}" LARGURA="${LARGURA:-900}" ALTURA="${ALTURA:-900}" PREFIXO="${PREFIXO:-farol}" \
  /opt/node22/bin/node "$AQUI/tela.mjs"
