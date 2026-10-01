#!/bin/sh
# BACKUP DA HISTORIA: gera o bundle git de uma branch e PROVA que ele restaura.
# Generalizado do `backup.sh` do PhxSql.
#
# USO (de dentro de qualquer repositorio git)
#   backup.sh                       # branch atual, pacote no diretorio-pai do repositorio
#   backup.sh BRANCH                # outra branch
#   backup.sh BRANCH DESTINO        # outro destino
#   KIT_PREFIXO=meuprojeto backup.sh  # prefixo do nome (padrao: nome do repositorio)
#
# Saida: DESTINO/<prefixo>-AAAAMMDD-HHMM.bundle, e as tres linhas do veredito.
#
# POR QUE RESTAURA, E NAO SO VERIFICA. Medido: cortados 2 MiB do fim de um
# bundle bom, `git bundle verify` disse «The bundle records a complete
# history» e saiu 0; `git clone` dele morreu com «index-pack died», saida 128.
# O verify le o CABECALHO, nao o packfile. Por isso este script clona de
# verdade e compara o SHA do objeto `tree` dos dois lados: tree igual quer
# dizer conteudo identico byte a byte de todo arquivo versionado.
set -eu

RAIZ=$(git rev-parse --show-toplevel)
BRANCH="${1:-$(git -C "$RAIZ" rev-parse --abbrev-ref HEAD)}"
DESTINO="${2:-$(cd "$RAIZ/.." && pwd)}"
PREFIXO="${KIT_PREFIXO:-$(basename "$RAIZ")}"
PACOTE="$DESTINO/$PREFIXO-$(date +%Y%m%d-%H%M).bundle"

# O PID no nome deixa um zelador saber que o diretorio e de processo vivo.
PROVA="${TMPDIR:-/tmp}/kit-prova-do-pacote-$$"
limpar() { rm -rf "$PROVA"; }
trap limpar EXIT INT TERM

git -C "$RAIZ" rev-parse --verify "$BRANCH" >/dev/null

echo "== gerando"
git -C "$RAIZ" bundle create "$PACOTE" "$BRANCH"

echo "== 1/3 cabecalho (git bundle verify)"
# Vale por si: pega pre-requisito faltando (pacote incompleto por construcao).
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
