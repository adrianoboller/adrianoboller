#!/bin/bash
# O video inteiro, refeito do zero por um comando so (root):
#   1. as tres janelas em netns (ambiente.sh)
#   2. colhe as medidas de hoje: autoteste, bancada, contagem de testes
#   3. grava os cartoes (cartoes.mjs) -- numeros lidos dos arquivos
#   4. corta as cenas e monta o MP4 (ffmpeg)
# Saida: $SAIDA (padrao /var/tmp/phx-demo/phxvpn-investidor.mp4)
set -euo pipefail
U=$(cd "$(dirname "$0")" && pwd); R=$(cd "$U/../.." && pwd)
D=/var/tmp/phx-demo; SAIDA=${SAIDA:-$D/phxvpn-investidor.mp4}
# SO_MONTAR=1: reaproveita janelas e medidas ja gravadas; refaz cartoes e MP4.
if [ -z "${SO_MONTAR:-}" ]; then
(cd "$R" && cargo build -q && cargo build -q --release)
"$U/ambiente.sh" | grep -E "erros|USB:"
"$R/target/release/phxvpncmd" /modo:ferramentas /comando:AUTOTESTE > $D/autoteste.txt 2>&1
"$R/target/release/phxvpncmd" /modo:ferramentas "/comando:BANCADA /segundos:1" > $D/bancada.txt 2>&1
conta() { grep -E "test result: ok. [0-9]+ passed" | head -1 | grep -o "[0-9]* passed" | cut -d' ' -f1; }
TL=$(cd "$R" && cargo test -q 2>&1 | conta)
TW=$(cd "$R" && CARGO_TARGET_X86_64_PC_WINDOWS_GNU_RUNNER=/usr/lib/wine/wine64 WINEDEBUG=-all cargo test -q --target x86_64-pc-windows-gnu 2>&1 | conta)
echo "{\"testes_linux\":${TL:-0},\"testes_windows\":${TW:-0}}" > $D/dados.json
fi
rm -f $D/parte-*.mp4
(cd "$U" && DEMO=$D FONTES=$U/fontes node cartoes.mjs | tail -1)

# Cada parte: 1280x720, 30 qps, H.264, com meio segundo de fade nas pontas.
N=0; LISTA=$D/partes.txt; : > $LISTA
parte() { # parte ARQUIVO [INI FIM]
  local ent=$1 dur; N=$((N+1)); local out=$D/parte-$(printf %02d $N).mp4
  if [ $# -eq 3 ]; then dur=$(python3 -c "print(round($3-$2,2))"); ARGS=(-ss "$2" -t "$dur")
  else dur=$(ffprobe -v error -show_entries format=duration -of csv=p=0 "$ent"); ARGS=(); fi
  local fim; fim=$(python3 -c "print(max(0.0, round($dur-0.5,2)))")
  ffmpeg -loglevel error -y "${ARGS[@]}" -i "$ent" -an \
    -vf "fps=30,scale=1280:720,format=yuv420p,fade=t=in:st=0:d=0.4,fade=t=out:st=$fim:d=0.4" \
    -c:v libx264 -preset veryfast -crf 22 "$out"
  echo "file '$out'" >> $LISTA
}
cena() { # cena QUEM NOME
  read -r INI FIM < <(python3 -c "import json;c=[x for x in json.load(open('$D/cenas-$1.json')) if x['nome']=='$2'][0];print(c['ini'],c['fim'])")
  parte "$D/bruto-$1.webm" "$INI" "$FIM"
}
parte $D/cartao-abertura.webm
cena a a1; cena b b1; cena a a2; cena b b2; cena a a3; cena b b3; cena a a4; cena c c1; cena a a5
parte $D/cartao-console.webm; parte $D/cartao-responsivo.webm; parte $D/cartao-fechamento.webm
ffmpeg -loglevel error -y -f concat -safe 0 -i $LISTA -c copy -movflags +faststart "$SAIDA"
echo "video: $SAIDA  $(du -h "$SAIDA" | cut -f1)  $(ffprobe -v error -show_entries format=duration -of csv=p=0 "$SAIDA" | cut -d. -f1) s"
