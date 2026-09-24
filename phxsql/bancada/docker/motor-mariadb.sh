#!/usr/bin/env bash
# O MariaDB de referencia (peso 3 na media ponderada dos quatro motores),
# em conteiner -- NUNCA por `apt`: medido, `apt-get install mariadb-server`
# REMOVE o mysql-server deste conteiner, e troca um motor de referencia
# pelo outro.
#
#   ./bancada/docker/motor-mariadb.sh sobe      # porta 127.0.0.1:3307
#   ./bancada/docker/motor-mariadb.sh estado
#   ./bancada/docker/motor-mariadb.sh derruba
#
# A imagem vem do ESPELHO oficial da Docker Library no Amazon ECR, e nao do
# Docker Hub: em 24/09/2026 o Hub respondeu 429 Too Many Requests ao pull
# anonimo por este proxy, e o espelho entregou a mesma imagem. Autorizacao
# do dono para o Docker: 24/09/2026.
#
# So escuta em 127.0.0.1 e o root nasce sem senha: e motor de TESTE,
# e so alcanca quem esta nesta maquina.
set -euo pipefail
IMAGEM=public.ecr.aws/docker/library/mariadb:11
NOME=phx-mariadb
PORTA=${PHX_MARIADB_PORTA:-3307}

garante_daemon() {
  if ! docker info >/dev/null 2>&1; then
    echo "== dockerd parado: subindo (log em /tmp/dockerd.log)"
    (dockerd >/tmp/dockerd.log 2>&1 &)
    for _ in $(seq 1 30); do docker info >/dev/null 2>&1 && return 0; sleep 1; done
    echo "dockerd nao subiu em 30 s -- veja /tmp/dockerd.log" >&2
    exit 1
  fi
}

case "${1:-estado}" in
  sobe)
    garante_daemon
    docker image inspect "$IMAGEM" >/dev/null 2>&1 || docker pull "$IMAGEM"
    if docker ps -a --format '{{.Names}}' | grep -qx "$NOME"; then
      docker start "$NOME" >/dev/null
    else
      docker run -d --name "$NOME" -e MARIADB_ALLOW_EMPTY_ROOT_PASSWORD=1 \
        -p "127.0.0.1:${PORTA}:3306" "$IMAGEM" >/dev/null
    fi
    for _ in $(seq 1 30); do
      if docker exec "$NOME" mariadb -uroot -e 'SELECT VERSION()' -N 2>/dev/null; then
        echo "MariaDB pronto em 127.0.0.1:${PORTA} (conteiner ${NOME})"; exit 0
      fi
      sleep 1
    done
    echo "o MariaDB nao respondeu em 30 s" >&2; exit 1 ;;
  estado)
    docker ps -a --filter "name=^${NOME}$" --format '{{.Names}} {{.Status}} {{.Ports}}' ;;
  derruba)
    docker rm -f "$NOME" >/dev/null && echo "derrubado: $NOME" ;;
  *) echo "uso: $0 sobe|estado|derruba" >&2; exit 2 ;;
esac
