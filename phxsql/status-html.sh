#!/bin/sh
# O comando da SETIMA PAGINA -- o status do projeto em HTML.
#
#   ./status-html.sh              # grava docs/status/status-do-projeto.html
#   ./status-html.sh saida.html   # grava em outro lugar (o canonico continua)
#   ./status-html.sh --conferir   # grava e roda o portao so para este gerador
#
# Ele e a PORTA, nao uma segunda implementacao: toda a montagem vive em
# `docs/status/pagina-do-status-do-projeto.py`. Receita duplicada e receita que
# diverge -- e uma segunda montagem aqui divergiria da primeira no dia em que
# alguem acrescentasse uma secao.
#
# ## A lei que este comando carrega, e ela custou caro
#
# *Gerador certo chamado pela metade entrega numero velho anunciando sucesso.*
# Em 07/09/2026 o `pagina-dos-pedidos.py` gravava a pagina, gravava a contagem,
# imprimia TRES linhas de exito e pulava o painel do dossie -- porque o alvo do
# painel so existia se viesse por argumento. Resultado: tres paineis atrasados
# sem um digito digitado (198 pedidos onde eram 203, 428 testes onde eram 451,
# 26.762 linhas/s onde eram 37.810).
#
# Por isso, aqui:
#
#   1. CHAMADA SEM ARGUMENTO FAZ A COISA INTEIRA. O alvo canonico nao depende
#      de alguem lembrar do caminho -- e a licao do `dossie_da_pasta.py`, que
#      varre a pasta em vez de guardar um nome digitado.
#
#   2. ELE DIZ QUE FEZ MENOS QUANDO FEZ MENOS. As secoes que NAO nasceram por
#      falta de gerador saem nomeadas, com o gerador que falta -- debaixo de um
#      cabecalho que nao e linha de exito. Secao que nao nasce APARECE; nao
#      some.
set -eu

cd "$(dirname "$0")"
RAIZ=$PWD
GERADOR="docs/status/pagina-do-status-do-projeto.py"
CONFERIR=0
SAIDA=""

for arg in "$@"; do
  case "$arg" in
    --conferir) CONFERIR=1 ;;
    --ajuda|-h) sed -n '2,6p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    -*) echo "status-html.sh: nao conheco a chave $arg" >&2; exit 2 ;;
    *) SAIDA=$arg ;;
  esac
done

[ -f "$GERADOR" ] || { echo "status-html.sh: nao achei $GERADOR" >&2; exit 2; }

echo "== gerando a setima pagina =="
# A saida do gerador ja traz a linha da pagina gravada E a lista do que nao
# nasceu -- ele nao imprime exito por aquilo que pulou.
if [ -n "$SAIDA" ]; then
  python3 "$GERADOR" "$SAIDA"
else
  python3 "$GERADOR"
fi

if [ "$CONFERIR" = "1" ]; then
  echo
  echo "== portao dos geradores, so este =="
  # Re-rodar mudaria algum numero visivel? Se mudar, o publicado estava VELHO.
  python3 docs/dossie/portao-dos-geradores.py --so docs/status/
fi

echo
echo "Publique passando a URL da pagina, para cair na mesma em vez de criar"
echo "outra -- a URL esta em docs/status/LEIA-ME.md."
