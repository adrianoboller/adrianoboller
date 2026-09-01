#!/bin/sh
# Gera o pacote de backup da branch e PROVA que ele restaura.
#
#   ./backup.sh [branch] [destino]
#
# Existe por duas regras da casa que se encontram aqui.
#
# A primeira: «pacote gerado por script, nunca montado a mao -- pacote feito a
# mao e pacote que ninguem consegue refazer igual». Este backup vinha saindo a
# mao, comando a comando, e por isso mesmo saia diferente a cada rodada.
#
# A segunda, e a que doeu: PROVA REAL NOS DOIS SENTIDOS. O procedimento
# anterior mandava conferir o pacote com `git bundle verify`, e isso esta
# ERRADO -- medido:
#
#   cortei 2 MiB do fim de um pacote bom
#   `git bundle verify`  -> «The bundle records a complete history», SAIDA 0
#   `git clone` dele     -> «error: index-pack died», SAIDA 128
#
# O `verify` confere o CABECALHO: quais refs o pacote traz e se a historia e
# auto-suficiente (nenhum commit-pre-requisito faltando). Ele nao le o
# packfile, entao nao ve conteudo corrompido nem truncado. Quem so roda o
# verify entrega backup podre com a consciencia limpa -- e um backup so
# reprova na hora de restaurar, que e a pior hora possivel.
#
# Por isso este script RESTAURA de verdade e compara a arvore. O criterio nao e
# «o clone nao deu erro»: e o SHA do objeto `tree` do HEAD ser o MESMO dos dois
# lados. Tree igual quer dizer conteudo identico byte a byte, para todo arquivo
# versionado -- e isso um clone que deu erro no meio nao consegue fingir.
set -eu

BRANCH="${1:-claude/capacidades-disponiveis-y6auxh}"
DESTINO="${2:-$(cd "$(dirname "$0")/.." && pwd)}"
RAIZ=$(git rev-parse --show-toplevel)
PACOTE="$DESTINO/phxsql-$(date +%Y%m%d-%H%M).bundle"

# O PID no nome e o que o zelador guarda: ele nunca apaga diretorio de processo
# vivo, e confere por caminho real, nunca por data ou nome.
PROVA="${TMPDIR:-/tmp}/phx-prova-do-pacote-$$"
limpar() { rm -rf "$PROVA"; }
trap limpar EXIT INT TERM

git -C "$RAIZ" rev-parse --verify "$BRANCH" >/dev/null

echo "== gerando"
git -C "$RAIZ" bundle create "$PACOTE" "$BRANCH"

echo "== 1/3 cabecalho (git bundle verify)"
# Vale por si: pega pre-requisito faltando, que e pacote INCOMPLETO por
# construcao -- outro defeito, e um que o clone tambem pegaria, mas mais tarde.
git bundle verify "$PACOTE" >/dev/null

echo "== 2/3 restaurando de verdade"
git clone -q --branch "$BRANCH" "$PACOTE" "$PROVA"

echo "== 3/3 comparando a arvore"
AQUI=$(git -C "$RAIZ" rev-parse "$BRANCH^{tree}")
LA=$(git -C "$PROVA" rev-parse "HEAD^{tree}")
if [ "$AQUI" != "$LA" ]; then
    echo "REPROVOU: a arvore restaurada nao e a mesma" >&2
    echo "  aqui: $AQUI" >&2
    echo "  la:   $LA" >&2
    rm -f "$PACOTE"
    exit 1
fi

COMMITS=$(git -C "$PROVA" rev-list --count HEAD)
echo
echo "PACOTE PROVADO: $PACOTE"
echo "  $(du -h "$PACOTE" | cut -f1), $COMMITS commits, arvore $AQUI"
echo "  ponta: $(git -C "$PROVA" rev-parse --short HEAD) $(git -C "$PROVA" log -1 --format=%s)"
