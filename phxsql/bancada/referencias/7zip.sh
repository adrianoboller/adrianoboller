#!/bin/sh
# Baixa o fonte do 7-Zip para consulta -- base de conhecimento do pedido 450
# e das funcoes internas do PhxSql, do PhxMail e do Phxblockchain.
#
# NAO e dependencia: nada daqui compila junto do produto. O fonte fica FORA
# do repositorio, porque metade dele e LGPL (ver docs/7ZIP.md) e porque o
# conteiner e efemero -- este script e o que faz a consulta sobreviver a
# sessao, e o commit fixado e o que faz duas sessoes lerem a mesma coisa.
#
# Uso: ./bancada/referencias/7zip.sh [destino]
set -eu

VERSAO=26.03
COMMIT=0766b733fe3e06dd2a7f9a3cfbf2108ac73abd17
DESTINO="${1:-/home/user/ip7z/7zip}"

if git -C "$DESTINO" rev-parse HEAD >/dev/null 2>&1; then
    atual=$(git -C "$DESTINO" rev-parse HEAD)
    if [ "$atual" = "$COMMIT" ]; then
        echo "ja esta la: $DESTINO em $VERSAO ($COMMIT)"
        exit 0
    fi
    # Outra versao no mesmo lugar: ler dela citando linha desta daria numero
    # de linha errado no documento. Quem decide trocar e quem chama.
    echo "RECUSADO: $DESTINO esta em $atual, e a consulta fixada e $COMMIT" >&2
    exit 1
fi

GIT_LFS_SKIP_SMUDGE=1 git -c advice.detachedHead=false clone -q --depth 1 --branch "$VERSAO" \
    https://github.com/ip7z/7zip "$DESTINO"

atual=$(git -C "$DESTINO" rev-parse HEAD)
if [ "$atual" != "$COMMIT" ]; then
    echo "RECUSADO: a tag $VERSAO aponta para $atual, e nao para $COMMIT" >&2
    exit 1
fi
echo "baixado: $DESTINO em $VERSAO ($COMMIT)"
