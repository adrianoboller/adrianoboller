#!/usr/bin/env bash
# Grava o video de demonstracao do PhxClaw: tela REAL (Xvfb) capturada com ffmpeg enquanto
# o desktop Tauri e os terminais rodam de verdade. Nada e simulado; o que ainda nao funciona
# aparece numa cartela propria no fim.
#
# Pre-requisitos (os mesmos dos gates): PostgreSQL em /tmp:55432 com tests/postgres/run_e2e.sh
# ja rodado, Ollama em 127.0.0.1:11434 com smollm2:135m, whisper.cpp em /var/tmp, binarios
# de exemplo e do desktop compilados. Saida: tools/demo/out/phxclaw-demo.mp4
set -eu
cd "$(dirname "$0")"
DEMO=$PWD; RAIZ=$(cd ../.. && pwd); OUT=$DEMO/out; mkdir -p "$OUT"; rm -f "$OUT"/*.mp4 "$OUT"/*.txt
W=1600; H=960; D=:99; FONTE=/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf; NEG=/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf
export DISPLAY=$D WEBKIT_DISABLE_DMABUF_RENDERER=1
export PHXCLAW_E2E_RLS_URL="host=/tmp port=55432 user=phx_rls dbname=phxclaw_e2e connect_timeout=2"
Xvfb $D -screen 0 ${W}x${H}x24 >/dev/null 2>&1 & XV=$!; sleep 1
openbox >/dev/null 2>&1 & OB=$!; sleep 1
trap 'kill $OB $XV 2>/dev/null || true' EXIT

n=0
cartela() { # titulo subtitulo segundos
  n=$((n + 1)); printf '%s' "$1" > "$OUT/t$n.txt"; printf '%s' "$2" > "$OUT/s$n.txt"
  ffmpeg -loglevel error -y -f lavfi -i "color=c=0x06101a:s=${W}x${H}:d=$3:r=25" -vf \
"drawtext=fontfile=$NEG:textfile=$OUT/t$n.txt:fontcolor=0xf5c64d:fontsize=64:x=(w-text_w)/2:y=h/2-90,\
drawtext=fontfile=$FONTE:textfile=$OUT/s$n.txt:fontcolor=0xa6bfd1:fontsize=28:x=(w-text_w)/2:y=h/2+10:line_spacing=14" \
    -c:v libx264 -pix_fmt yuv420p "$OUT/$(printf %02d $n).mp4"
}
grava() { # rotulo comando...
  n=$((n + 1)); local rot=$1; shift
  printf '%s' "$rot" > "$OUT/r$n.txt"
  ffmpeg -loglevel error -y -f x11grab -video_size ${W}x${H} -framerate 25 -i $D \
    -c:v libx264 -preset veryfast -pix_fmt yuv420p "$OUT/cru$n.mp4" & local FF=$!
  sleep 0.5; "$@" || true; sleep 1
  kill -INT $FF; wait $FF || true
  # faixa inferior com o que a cena prova
  ffmpeg -loglevel error -y -i "$OUT/cru$n.mp4" -vf \
"drawbox=x=0:y=ih-54:w=iw:h=54:color=0x000000@0.72:t=fill,\
drawtext=fontfile=$FONTE:textfile=$OUT/r$n.txt:fontcolor=0xf5c64d:fontsize=24:x=24:y=h-39" \
    -c:v libx264 -pix_fmt yuv420p "$OUT/$(printf %02d $n).mp4"
  rm -f "$OUT/cru$n.mp4"
}
# -u8 + locale UTF-8: sem os dois, o xterm mostrava 'instalaÃ§Ã£o' (medido na 1a gravacao).
# cursor do mouse fora da area de texto (na 2a gravacao ficou no meio da tela)
terminal() { xdotool mousemove $((W - 1)) $((H - 1)); LANG=C.UTF-8 LC_ALL=C.UTF-8 xterm -u8 -geometry 124x33+0+0 -fa 'DejaVu Sans Mono' -fs 17 \
  -bg '#07121d' -fg '#d5e3ee' -b 18 -T PhxClaw -e bash "$DEMO/$1"; }

cartela "PhxClaw v0.70" "Plataforma de agentes de IA local-first em Rust
o que JÁ funciona, rodando de verdade — gravação de tela, sem simulação" 5
cartela "1 · Host desktop (Tauri)" "binário nativo, ponte IPC viva, política que nega tudo por padrão" 3
grava "Tauri real: Command Center, ponte IPC, evidência encadeada e execute_shell NEGADO pela política" \
  env PHXCLAW_E2E_PAUSA=4 python3 "$RAIZ/tests/desktop/desktop_e2e.py"
cartela "2 · Banco de dados" "PostgreSQL 16 real, 340 tabelas, isolamento por cliente" 3
grava "PostgreSQL 16.13 real: 16 baterias E2E, RLS provado com papel sem privilégio" terminal cena_postgres.sh
cartela "3 · Resiliência" "o banco morre no meio do trabalho — e nada confirmado se perde" 3
grava "Caos real: SIGKILL repetido no PostgreSQL, 0 decisões perdidas, 0 divergências" terminal cena_caos.sh
cartela "4 · IA local" "modelo de linguagem e reconhecimento de voz, sem nuvem" 3
grava "Ollama + whisper.cpp reais, chamados pelos adaptadores do PhxClaw" terminal cena_ia.sh
cartela "5 · Segurança" "auditoria de dependências e provas de ataque" 3
grava "cargo audit 0 vulnerabilidades; SSRF, vazamento de token e segredo no argv barrados" terminal cena_seguranca.sh
cartela "6 · Certificação" "o relatório que diz o que passou — e o que ainda não" 3
grava "Certificação gerada: 7/12 gates obrigatórios, bloqueios com motivo" terminal cena_certificacao.sh
cartela "O que ainda NÃO funciona" "assinatura dos 6 plugins internos (depende da chave do dono)
canais Telegram/Discord/Slack/WhatsApp/Teams sem credenciais · dispositivos físicos
OpenAI/Anthropic/Gemini só montam a requisição · 76 de 113 módulos sem executável que os use" 9

ls "$OUT"/[0-9][0-9].mp4 | sed "s/^/file '/; s/$/'/" > "$OUT/lista.txt"
ffmpeg -loglevel error -y -f concat -safe 0 -i "$OUT/lista.txt" -c:v libx264 -crf 22 -preset medium \
  -pix_fmt yuv420p -movflags +faststart "$OUT/phxclaw-demo.mp4"
rm -f "$OUT"/[0-9][0-9].mp4 "$OUT"/*.txt
ffprobe -v error -show_entries format=duration,size -of default=nw=1 "$OUT/phxclaw-demo.mp4"
