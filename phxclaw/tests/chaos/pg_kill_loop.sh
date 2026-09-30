#!/usr/bin/env bash
# Mata o PostgreSQL com SIGKILL em intervalos aleatorios e o sobe de novo, por N segundos.
# SIGKILL no postmaster e o pior caso que um processo sofre: sem checkpoint, sem aviso.
# A recuperacao e o WAL do PostgreSQL; o que se prova e que o MOTOR nao confirma o que
# o banco nao gravou, e nao grava pela metade (ver crates/phxclaw-hypothesis/tests/pg_chaos.rs).
#
# Uso: PGDATA=/var/tmp/phxclaw-pg PGPORT=55432 tests/chaos/pg_kill_loop.sh 40
set -u
DUR=${1:-30}; PGDATA=${PGDATA:-/var/tmp/phxclaw-pg}; PGPORT=${PGPORT:-55432}
PGBIN=${PGBIN:-/usr/lib/postgresql/16/bin}
fim=$((SECONDS + DUR)); mortes=0
while [ $SECONDS -lt $fim ]; do
  sleep "$(awk 'BEGIN{srand(); printf "%.1f", 2 + rand()*3}')"
  pid=$(head -1 "$PGDATA/postmaster.pid" 2>/dev/null) || continue
  kill -9 "$pid" 2>/dev/null && mortes=$((mortes + 1)) && echo "SIGKILL #$mortes no postmaster (pid $pid) -- religando"
  # Filhos orfaos seguram a memoria compartilhada e o lock do socket: so sobe de novo
  # quando nenhum processo do cluster sobrou (um init de verdade faz o mesmo).
  for _ in $(seq 50); do
    pkill -9 -u postgres 2>/dev/null
    pgrep -u postgres >/dev/null || break
    sleep 0.1
  done
  rm -f "$PGDATA/postmaster.pid" "/tmp/.s.PGSQL.$PGPORT.lock"
  su postgres -c "$PGBIN/pg_ctl -D $PGDATA -o '-p $PGPORT -k /tmp' -l $PGDATA/log start -w" >/dev/null 2>&1
done
echo "sigkill: $mortes"
