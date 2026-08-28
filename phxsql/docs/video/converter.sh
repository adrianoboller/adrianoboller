#!/bin/bash
# Converte o WebM do Playwright em MP4 (H.264), que e o que abre em tudo.
set -e
S="$(dirname "$0")"
BRUTO=$(ls "$S"/bruto/*.webm | head -1)
SAIDA="$S/phxsql-0.15-demo.mp4"
ffmpeg -y -i "$BRUTO" \
  -c:v libx264 -preset slow -crf 22 -pix_fmt yuv420p \
  -vf "scale=1600:900:flags=lanczos,fps=24" \
  -movflags +faststart -an "$SAIDA" 2>&1 | tail -3
ls -la "$SAIDA"
ffprobe -v error -show_entries format=duration,size -of default=noprint_wrappers=1 "$SAIDA"
