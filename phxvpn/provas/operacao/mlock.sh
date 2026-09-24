#!/usr/bin/env bash
# Prova do `mlock` (chave fora do swap) contra o /proc, com tres binarios:
#
#   velho       -- sem mlock nenhum (o RED: VmLck 0)
#   novo        -- mlockall(MCL_CURRENT|MCL_FUTURE|MCL_ONFAULT)
#   sem-onfault -- o mesmo SEM MCL_ONFAULT (a hipotese que perdeu, se perder)
#
# Mede VmLck e VmRSS do repasse parado e do painel parado e com 120 conexoes
# abertas ao mesmo tempo (cada uma e uma thread com pilha propria: e ali que
# travar o reservado custa). E o caso sem permissao: repasse como `nobody` com
# o limite de 8 MiB tem de NAO travar e continuar vivo.
#
# Tudo num netns proprio (porta nenhuma do hospedeiro); mata por PID.
# Uso: sudo PHXVPN_BIN_VELHO=... PHXVPN_BIN_SEM_ONFAULT=... ./mlock.sh
set -euo pipefail
AQUI=$(cd "$(dirname "$0")" && pwd)
RAIZ=$(cd "$AQUI/../.." && pwd)
BIN_NOVO=${PHXVPN_BIN:-$RAIZ/target/release/phxvpn}
BIN_VELHO=${PHXVPN_BIN_VELHO:?informe PHXVPN_BIN_VELHO}
BIN_SEM=${PHXVPN_BIN_SEM_ONFAULT:?informe PHXVPN_BIN_SEM_ONFAULT}
PGBIN=$(ls -d /usr/lib/postgresql/*/bin | sort -V | tail -1)
NS=pxml
PGPORTA=55463

limpar() {
  set +e
  ip netns pids $NS 2>/dev/null | xargs -r kill
  sleep 1
  [ -n "${T:-}" ] && [ -d "$T/pg" ] && ip netns exec $NS su postgres -c "$PGBIN/pg_ctl -D $T/pg -m fast stop" >/dev/null 2>&1
  ip netns pids $NS 2>/dev/null | xargs -r kill -9
  ip netns del $NS 2>/dev/null
}
trap limpar EXIT
limpar
set -e
ip netns add $NS; ip -n $NS link set lo up
T=$(mktemp -d /tmp/phxvpn-prova-mlock.XXXX); chmod 755 "$T"
mkdir -p "$T/pg" "$T/pgsock"; chown postgres "$T/pg" "$T/pgsock"
echo "senha-pg-prova" > "$T/pgsenha"; chown postgres "$T/pgsenha"
su postgres -c "$PGBIN/initdb -D $T/pg -A scram-sha-256 --pwfile=$T/pgsenha -U postgres" >/dev/null
ip netns exec $NS su postgres -c "$PGBIN/pg_ctl -D $T/pg -o '-p $PGPORTA -k $T/pgsock -c listen_addresses=127.0.0.1' -l $T/pg/pg.log start" >/dev/null
sleep 2

campo() { awk -v c="$2:" '$1 == c {print $2}' /proc/$1/status; }
medir() { echo "{\"vmlck_kib\": $(campo $1 VmLck), \"vmrss_kib\": $(campo $1 VmRSS), \"threads\": $(campo $1 Threads)}"; }

repasse() { # ROTULO BINARIO [setpriv...]
  local r=$1 b=$2; shift 2
  (cd "$T" && "$b" p2p chave --arquivo "$T/r-$r.chave" >/dev/null)
  chmod 644 "$T/r-$r.chave"
  ip netns exec $NS "$@" "$b" repasse --porta 5190$((RANDOM % 9)) --chave "$T/r-$r.chave" > "$T/rep-$r.log" 2>&1 &
  local pid=$!
  sleep 1.5
  local vivo=false; kill -0 $pid 2>/dev/null && vivo=true
  local m; m=$(medir $pid 2>/dev/null || echo '{}')
  kill $pid 2>/dev/null; wait $pid 2>/dev/null || true
  local aviso=false; grep -q "AVISO repasse: chaves podem ir ao swap" "$T/rep-$r.log" && aviso=true
  python3 -c "import json,sys; d=json.loads(sys.argv[1] or '{}'); d.update(vivo=sys.argv[2]=='true', aviso_no_log=sys.argv[3]=='true'); print(json.dumps(d))" "$m" $vivo $aviso
}

painel() { # ROTULO BINARIO
  local r=$1 b=$2
  S_PG="host=127.0.0.1 port=$PGPORTA user=postgres password=senha-pg-prova dbname=postgres"
  ip netns exec $NS env PHXVPN_PG="$S_PG" "$b" painel --dados "$T/dados-$r" --escutar 127.0.0.1:8494 > "$T/painel-$r.log" 2>&1 &
  local pid=$!
  for _ in $(seq 50); do grep -q "painel em" "$T/painel-$r.log" && break; sleep 0.2; done
  sleep 1
  local parado; parado=$(medir $pid)
  # 120 conexoes abertas e mudas ao mesmo tempo: cada uma segura uma thread.
  ip netns exec $NS python3 - <<'PY' &
import socket, time
s = []
for _ in range(120):
    c = socket.create_connection(("127.0.0.1", 8494)); c.send(b"GET / HTTP/1.1\r\n"); s.append(c)
time.sleep(4)
PY
  local py=$!
  sleep 2.5
  local carga; carga=$(medir $pid)
  wait $py 2>/dev/null || true
  kill $pid; wait $pid 2>/dev/null || true
  echo "{\"parado\": $parado, \"com_120_conexoes\": $carga}"
}

R=$T/res.json
{
  echo "{"
  for par in velho:$BIN_VELHO novo:$BIN_NOVO sem-onfault:$BIN_SEM; do
    r=${par%%:*}; b=${par#*:}
    echo "\"$r\": {\"repasse_root\": $(repasse $r-root $b), \"painel_root\": $(painel $r $b)},"
  done
  # Sem permissao: nobody, limite de 8 MiB, sem CAP_IPC_LOCK.
  echo "\"novo_nobody_limite_8mib\": $(repasse nobody $BIN_NOVO setpriv --reuid=65534 --regid=65534 --clear-groups --inh-caps=-all --bounding-set=-all)"
  echo "}"
} > "$R"
python3 - "$R" "$AQUI" "$(ulimit -Sl)" "$(ulimit -Hl)" "$(awk '/^CapEff:/ {print $2}' /proc/self/status)" <<'PY'
import json, sys, datetime, platform, subprocess, os
d = json.load(open(sys.argv[1]))
cap = int(sys.argv[5], 16)
v, n, s = d["velho"], d["novo"], d["sem-onfault"]
nb = d["novo_nobody_limite_8mib"]
ok = (v["repasse_root"]["vmlck_kib"] == 0 and v["painel_root"]["parado"]["vmlck_kib"] == 0
      and n["repasse_root"]["vmlck_kib"] > 0 and n["painel_root"]["parado"]["vmlck_kib"] > 0
      and nb["vivo"] and nb["aviso_no_log"] and nb["vmlck_kib"] == 0)
r = {
  "prova": "mlockall no repasse e no painel, conferido no /proc/<pid>/status",
  "medido_em": datetime.datetime.now(datetime.timezone.utc).isoformat(timespec="seconds"),
  "kernel": platform.release(), "n": 1,
  "ambiente": {"rlimit_memlock_macio": sys.argv[3], "rlimit_memlock_duro": sys.argv[4],
               "cap_ipc_lock": bool(cap >> 14 & 1), "cap_sys_resource": bool(cap >> 24 & 1)},
  "medidas": d,
  "confere": ok,
}
f = sys.argv[2] + "/secao-mlock.tmp.json"
json.dump(r, open(f, "w"), ensure_ascii=False, indent=1)
subprocess.run([sys.executable, sys.argv[2] + "/juntar.py", "mlock", f], check=True)
os.remove(f)
print(json.dumps(r, ensure_ascii=False, indent=1))
sys.exit(0 if ok else 1)
PY
