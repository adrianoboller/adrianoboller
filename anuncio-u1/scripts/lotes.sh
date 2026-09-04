#!/usr/bin/env bash
# Previa completa em lotes: cada lote e um processo do Blender separado
# (< 10 min), quadros impares 1..749 de 2 em 2 (15 fps efetivo; 375 quadros
# dos 750 = 25 s) a 360x640 e 8 amostras -> saida/previa_seq/quadro_NNNN.png.
# Depois:
#   NOME_VIDEO=previa_25s.mp4 MODO=video bash scripts/previa.sh scripts/teste_coreografia.py
# junta tudo num MP4 mudo; com som e a prova do MP4:
#   bash scripts/previa.sh scripts/video_com_som.py
#
# Custo medido: 16-41 s por quadro por software; 14 quadros por lote ficam
# abaixo dos 10 min, e a sequencia inteira leva ~3 h (27 lotes). INI/FIM/TAM
# (quadros por lote) e CAIXA_SOME passam por variavel de ambiente - um lote
# curto de prova e INI=1 FIM=9 (cinco quadros); os logs vao para
# saida/previa_seq/logs/.
cd "$(dirname "$0")/.."
LOGS=saida/previa_seq/logs
mkdir -p "$LOGS"
INI=${INI:-1}
ULTIMO=${FIM:-749}
TAM=${TAM:-14}
while [ "$INI" -le "$ULTIMO" ]; do
  FIM_LOTE=$((INI + 2 * (TAM - 1)))
  [ "$FIM_LOTE" -gt "$ULTIMO" ] && FIM_LOTE=$ULTIMO
  T0=$(date +%s)
  MODO=lote INI=$INI FIM=$FIM_LOTE PASSO=2 timeout 590 bash scripts/previa.sh scripts/teste_coreografia.py > "$LOGS/lote_$INI.log" 2>&1
  echo "lote $INI..$FIM_LOTE exit=$? em $(( $(date +%s) - T0 )) s" >> "$LOGS/progresso.txt"
  INI=$((FIM_LOTE + 2))
done
echo "FIM" >> "$LOGS/progresso.txt"
