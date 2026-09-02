#!/usr/bin/env bash
# Renderiza com EEVEE Next sem GPU, via Xvfb + llvmpipe (Mesa por software).
#
# Existe porque o container nao tem placa de video e o EEVEE exige contexto
# OpenGL: sem isto ele aborta em "Couldn't open libEGL.so.1". Com isto, a
# previa daqui sai do MESMO motor que o render final na RTX 4050 do Adriano -
# o que muda tudo: material e luz aprovados na previa nao mudam de cara no
# final. (Cycles CPU ficaria parecido, nao igual.)
#
# Uso: previa.sh <script.py> [-- args para o script]
#      previa.sh --blend cena.blend <script.py>
#
# Custo medido (llvmpipe, 4 nucleos): ~6 s por quadro para um corpo de prova
# simples (o cubo dos testes de modulo) em 540x960/16. A CENA COMPLETA do
# anuncio custa 27-40 s por quadro, tanto a 360x640/8 quanto a 540x960/16 -
# por isso os lotes da previa (lotes.sh) tem 14 quadros, para caber em 10 min.

set -euo pipefail

BLENDER="${BLENDER:-/tmp/claude-0/-home-user-adrianoboller/857c9466-78dd-51a3-8b8e-9f13227d5fd4/scratchpad/blender/blender}"
BLEND=""
if [ "${1:-}" = "--blend" ]; then
  BLEND="$2"; shift 2
fi
SCRIPT="$1"; shift

export LIBGL_ALWAYS_SOFTWARE=1
export GALLIUM_DRIVER=llvmpipe
# llvmpipe usa todos os nucleos por padrao; o LP_NUM_THREADS trava isso para o
# render nao brigar com outro processo de render rodando ao lado.
export LP_NUM_THREADS="${LP_NUM_THREADS:-4}"

if [ -n "$BLEND" ]; then
  exec xvfb-run -a -s "-screen 0 1280x720x24" "$BLENDER" -b "$BLEND" -P "$SCRIPT" "$@"
else
  exec xvfb-run -a -s "-screen 0 1280x720x24" "$BLENDER" -b -P "$SCRIPT" "$@"
fi
