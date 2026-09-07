#!/bin/bash
# A prova da guarda da corrente do batimento -- nos TRES estados.
#
#   ./prova-da-guarda-do-pulso.sh
#
# O batimento fino de 15 min e uma corrente de tiros unicos: cada elo forja o
# seguinte. Em 07/09/2026 o conteiner reiniciou no meio de um turno, o elo
# seguinte nunca nasceu, e a corrente ficou ~3 h parada EM SILENCIO -- o buraco
# que o pedido 204 declarava. A guarda mora no `comunicacao.sh`.
#
# Prova real nos dois sentidos, e aqui sao tres porque ha tres estados:
#
#   sem marca    a maquina reiniciou -- o elo em voo morreu com o conteiner, e
#                marca ausente NAO pode passar por «tudo bem»
#   marca velha  a corrente arrebentou: diz quantos minutos, com o teto
#   marca fresca a corrente esta viva -- e a guarda tem de FICAR CALADA.
#                Guarda que reclama sempre nao protege nada, e este terceiro
#                caso e o que separa uma guarda de um alarme quebrado.
set -u
AQUI="$(cd "$(dirname "$0")" && pwd)"
PULSO="${TMPDIR:-/tmp}/phx-batimento-pulso"
GUARDADO=""
[ -f "$PULSO" ] && GUARDADO="$(cat "$PULSO")"
FALHAS=0

ok() { if [ "$2" = 1 ]; then echo "  OK     $1"; else echo "  FALHA  $1"; FALHAS=$((FALHAS+1)); fi; }

echo "== 1. sem marca: a maquina reiniciou"
rm -f "$PULSO"
S="$("$AQUI/comunicacao.sh" 2>&1)"
echo "$S" | grep -q "sem marca de pulso" && ok "avisa que a corrente precisa ser refeita" 1 \
                                         || ok "avisa que a corrente precisa ser refeita" 0

echo "== 2. marca velha: 3 h, como aconteceu de verdade"
echo $(( $(date +%s) - 10800 )) > "$PULSO"
S="$("$AQUI/comunicacao.sh" 2>&1)"
echo "$S" | grep -q "SEM RESPOSTA ha 180 min" && ok "acusa, com o numero de minutos" 1 \
                                          || ok "acusa, com o numero de minutos" 0

echo "== 3. marca fresca: a corrente viva -- a guarda tem de CALAR"
echo $(( $(date +%s) - 300 )) > "$PULSO"
S="$("$AQUI/comunicacao.sh" 2>&1)"
if echo "$S" | grep -qE "SEM RESPOSTA|sem marca de pulso"; then
  ok "cala quando a corrente esta viva" 0
else
  ok "cala quando a corrente esta viva" 1
fi

# Devolve a marca que estava la: um portao que suja o estado de quem o chamou
# faz o proximo aviso mentir.
if [ -n "$GUARDADO" ]; then echo "$GUARDADO" > "$PULSO"; else rm -f "$PULSO"; fi

echo
[ "$FALHAS" = 0 ] && { echo "a guarda do pulso pega, e sabe calar."; exit 0; }
echo "$FALHAS FALHA(S)"; exit 1
