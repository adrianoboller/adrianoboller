#!/usr/bin/env bash
# Prova da guarda de isolamento SEM rota nenhuma: o host do servidor ja
# encaminha por fora (ip_forward=1, como com Docker ou num roteador), o painel
# sobe duas redes e nenhuma rota. A rede Outra nao alcanca a Matriz; RED sem
# a tabela, alcanca. E a tabela so existe com rede: antes da primeira, nao
# existe; depois de a ultima sair (pelo banco -- o painel nao tem «remover
# rede», porque rede com membro nao se apaga) e o painel subir de novo, some.
#
#   S servidor (painel, PostgreSQL, os openvpn)   A «ana» na Matriz
#   X «xavier» na Outra
#
# Uso (root):  MOTOR=nft ./guarda.sh   |   MOTOR=iptables ./guarda.sh
# Grava a chave "guarda_sem_rota_<motor>" em resultados.json, ao lado.
set -euo pipefail

AQUI=$(cd "$(dirname "$0")" && pwd)
RAIZ=$(cd "$AQUI/../.." && pwd)
MOTOR=${MOTOR:-nft}
T=${1:-$(mktemp -d /tmp/phxvpn-prova-guarda.XXXX)}
BIN=${PHXVPN_BIN:-$RAIZ/target/debug/phxvpn}
PGBIN=$(ls -d /usr/lib/postgresql/*/bin | sort -V | tail -1)
PGPORTA=55496
PAINEL=127.0.0.1:8483
P=pxgd
NSS="$P-s $P-a $P-x"
mkdir -p "$T"; chmod 755 "$T"
echo "== pasta: $T (motor $MOTOR)"

limpar() {
  set +e
  for n in $P-a $P-x $P-s; do ip netns pids $n 2>/dev/null | xargs -r kill; done
  sleep 1
  [ -d "$T/pg" ] && ip netns exec $P-s su postgres -c "$PGBIN/pg_ctl -D $T/pg -m fast stop" >/dev/null 2>&1
  for n in $NSS; do ip netns pids $n 2>/dev/null | xargs -r kill -9; ip netns del $n 2>/dev/null; rm -rf /etc/netns/$n; done
}
trap limpar EXIT
limpar; set -e
ns() { local n=$1; shift; ip netns exec "$P-$n" "$@"; }
for n in $NSS; do ip netns add $n; ip -n $n link set lo up; done
par() { # par NS_A IF_A IP_A NS_B IF_B IP_B
  ip link add "$2" netns "$P-$1" type veth peer name "$5" netns "$P-$4"
  ns "$1" ip addr add "$3" dev "$2"; ns "$1" ip link set "$2" up
  ns "$4" ip addr add "$6" dev "$5"; ns "$4" ip link set "$5" up
}
par s sa0 192.168.89.1/24 a eth0 192.168.89.11/24
par s sx0 192.168.87.1/24 x eth0 192.168.87.11/24
for n in a:192.168.89.1 x:192.168.87.1; do
  mkdir -p /etc/netns/$P-${n%%:*}; echo "${n#*:} vpn.prova.local" > /etc/netns/$P-${n%%:*}/hosts
done
# O host JA encaminha, por fora do phxvpn.
ns s sh -c 'echo 1 > /proc/sys/net/ipv4/ip_forward'

mkdir -p "$T/pg" "$T/pgsock"; chown postgres "$T/pg" "$T/pgsock"
echo "senha-pg-prova" > "$T/pgsenha"; chown postgres "$T/pgsenha"
su postgres -c "$PGBIN/initdb -D $T/pg -A scram-sha-256 --pwfile=$T/pgsenha -U postgres" >/dev/null
ns s su postgres -c "$PGBIN/pg_ctl -D $T/pg -o '-p $PGPORTA -k $T/pgsock -c listen_addresses=127.0.0.1' -l $T/pg/pg.log start" >/dev/null
sleep 2
PG="host=127.0.0.1 port=$PGPORTA user=postgres password=senha-pg-prova dbname=postgres"
MESTRE=senha-mestre-longa-da-prova
subir_painel() { # [com a senha mestre no ambiente]
  : > "$T/painel.log"
  ns s env PHXVPN_FIREWALL=$MOTOR PHXVPN_PG="$PG" ${1:+PHXVPN_SENHA_MESTRE=$MESTRE} \
    "$BIN" painel --dados "$T/dados" --openvpn --escutar $PAINEL >>"$T/painel.log" 2>&1 &
  PID_PAINEL=$!
  for _ in $(seq 100); do grep -q "painel em http" "$T/painel.log" && return 0; sleep 0.2; done
  echo "FALHOU: painel nao subiu"; tail "$T/painel.log"; exit 1
}
subir_painel
CODIGO=$(grep -o "CODIGO DE INSTALACAO: [^ ]*" "$T/painel.log" | awk '{print $4}')
api() {
  ns s python3 - "$@" <<'PY'
import json, sys, urllib.request
m, c, tk, corpo = sys.argv[1:5]
r = urllib.request.Request("http://127.0.0.1:8483" + c, method=m,
    data=corpo.encode() if m == "POST" else None,
    headers={"Content-Type": "application/json", **({"Authorization": "Bearer " + tk} if tk else {})})
try:
    print(urllib.request.urlopen(r, timeout=60).read().decode())
except urllib.error.HTTPError as e:
    print(e.read().decode()); sys.exit(1)
PY
}
campo() { python3 -c "import json,sys; print(json.load(sys.stdin)$1)"; }
tabela() { # regras nossas (0 = sem tabela/cadeia) e se a tabela/cadeia existe
  if [ "$MOTOR" = nft ]; then
    local t; t=$(ns s nft -a list table ip phxvpn 2>/dev/null || true)
    echo "{\"existe\": $([ -n "$t" ] && echo true || echo false), \"regras\": $(echo "$t" | grep '# handle' | grep -vc -e 'chain ' -e 'table ' || true), \"ip_forward\": $(ns s cat /proc/sys/net/ipv4/ip_forward)}"
  else
    local e=false; ns s iptables -S PHXVPN-ENC >/dev/null 2>&1 && e=true
    echo "{\"existe\": $e, \"regras\": $( { ns s iptables -S PHXVPN-ENC 2>/dev/null; ns s iptables -t nat -S PHXVPN-NAT 2>/dev/null; } | grep -c '^-A' || true), \"ip_forward\": $(ns s cat /proc/sys/net/ipv4/ip_forward)}"
  fi
}
api POST /api/instalar "" "{\"codigo_instalacao\":\"$CODIGO\",\"empresa\":\"Prova Ltda\",\"responsavel\":\"P\",\"email\":\"p@prova.local\",
 \"admin_usuario\":\"admin\",\"admin_senha\":\"senha-admin-longa\",\"senha_mestre\":\"$MESTRE\",
 \"servidor_nome\":\"vpn.prova.local\",\"servidor_ip\":\"192.168.89.1\",\"servidor_dns\":\"vpn.prova.local\"}" >/dev/null
TK=$(api POST /api/login "" '{"usuario":"admin","senha":"senha-admin-longa"}' | campo "['token']")
SEM_REDE=$(tabela); echo "== instalado, sem rede: $SEM_REDE"
api POST /api/redes "$TK" '{"nome":"Matriz","senha":"senha-da-matriz"}' >/dev/null
UMA_REDE=$(tabela); echo "== primeira rede: $UMA_REDE"
api POST /api/redes "$TK" '{"nome":"Outra","senha":"senha-da-outra"}' >/dev/null
membro() {
  api POST /api/usuarios "$TK" "{\"login\":\"$1\",\"senha\":\"senha-de-$1-longa\"}" >/dev/null
  local tk; tk=$(api POST /api/login "" "{\"usuario\":\"$1\",\"senha\":\"senha-de-$1-longa\"}" | campo "['token']")
  api POST /api/redes/entrar "$tk" "{\"nome\":\"$2\",\"senha\":\"$3\"}" | campo "['perfil']" > "$T/$1.ovpn"
}
membro ana Matriz senha-da-matriz
membro xavier Outra senha-da-outra
DUAS_REDES=$(tabela); echo "== duas redes, nenhuma rota: $DUAS_REDES"
sleep 1
conectar() {
  local log="$T/c-$2.log"
  ns "$1" openvpn --config "$T/$2.ovpn" --daemon --log "$log" --writepid "$T/c-$2.pid"
  for _ in $(seq 100); do grep -q "Initialization Sequence Completed" "$log" 2>/dev/null && return 0; sleep 0.2; done
  echo "FALHOU: $2 nao conectou"; tail -5 "$log"; exit 1
}
ip_tun() { ns "$1" ip -4 -o addr show dev tun0 | awk '{print $4}' | cut -d/ -f1; }
pings() { local n; n=$(ns "$1" ping -c 3 -W 1 -i 0.3 "$2" 2>/dev/null | awk '/received/ {print $4}' || true); echo "${n:-0}"; }
conectar a ana; conectar x xavier
IP_A=$(ip_tun a); IP_X=$(ip_tun x)
ns x ip route add "${IP_A%.*}.0/24" dev tun0
ns a ip route add "${IP_X%.*}.0/24" dev tun0
ISO_GUARDA=$(pings x "$IP_A")
if [ "$MOTOR" = nft ]; then ns s nft delete table ip phxvpn; else ns s iptables -D FORWARD -j PHXVPN-ENC; fi
ISO_RED=$(pings x "$IP_A")
echo "== X(Outra)->A(Matriz), sem rota nenhuma, ip_forward=1 por fora: com a guarda $ISO_GUARDA/3; RED sem ela $ISO_RED/3"

# A ultima rede sai: o painel nao tem «remover rede» (rede com membro nao se
# apaga), entao sai pelo banco, e o painel sobe de novo.
for p in $(ip netns pids $P-s); do
  tr '\0' ' ' < /proc/$p/cmdline 2>/dev/null | grep -q -e "phxvpn painel" -e "openvpn --config" && kill "$p"
done
wait "$PID_PAINEL" 2>/dev/null || true
ns s env PGPASSWORD=senha-pg-prova "$PGBIN/psql" -h 127.0.0.1 -p $PGPORTA -U postgres -d postgres -q \
  -c "DELETE FROM phx_rota; DELETE FROM phx_membro; DELETE FROM phx_revogado; DELETE FROM phx_rede"
# O kernel ainda tem a tabela da vida anterior do painel (o RED so tirou o salto no iptables).
if [ "$MOTOR" = nft ]; then printf 'table ip phxvpn {\n chain encaminhar {\n  type filter hook forward priority 0; policy accept;\n }\n}\n' | ns s nft -f -; fi
ANTES_DE_SUBIR=$(tabela)
subir_painel com-mestre
SEM_REDES_DE_NOVO=$(tabela); echo "== ultima rede fora, painel de novo: antes $ANTES_DE_SUBIR -> depois $SEM_REDES_DE_NOVO"

export SEM_REDE UMA_REDE DUAS_REDES ISO_GUARDA ISO_RED ANTES_DE_SUBIR SEM_REDES_DE_NOVO
python3 - "$AQUI/resultados.json" "$MOTOR" <<'PY'
import json, sys, os, datetime
caminho, motor = sys.argv[1], sys.argv[2]
E = os.environ
r = {
  "data": datetime.datetime.now(datetime.timezone.utc).isoformat(timespec="seconds"),
  "ip_forward_do_host": "1, posto por fora do phxvpn antes do painel",
  "instalado_sem_rede": json.loads(E["SEM_REDE"]),
  "primeira_rede": json.loads(E["UMA_REDE"]),
  "duas_redes_sem_rota": json.loads(E["DUAS_REDES"]),
  "outra_para_matriz_com_guarda": E["ISO_GUARDA"] + "/3",
  "outra_para_matriz_RED_sem_guarda": E["ISO_RED"] + "/3",
  "ultima_rede_fora_antes_de_subir": json.loads(E["ANTES_DE_SUBIR"]),
  "ultima_rede_fora_painel_de_novo": json.loads(E["SEM_REDES_DE_NOVO"]),
}
ok = (not r["instalado_sem_rede"]["existe"] and r["primeira_rede"]["existe"]
  and r["duas_redes_sem_rota"]["regras"] > 0 and r["duas_redes_sem_rota"]["ip_forward"] == 1
  and r["outra_para_matriz_com_guarda"] == "0/3" and r["outra_para_matriz_RED_sem_guarda"] == "3/3"
  and r["ultima_rede_fora_antes_de_subir"]["existe"] and not r["ultima_rede_fora_painel_de_novo"]["existe"]
  and r["ultima_rede_fora_painel_de_novo"]["ip_forward"] == 1)
r["passou"] = ok
tudo = json.load(open(caminho)) if os.path.exists(caminho) else {}
tudo["guarda_sem_rota_" + motor] = r
json.dump(tudo, open(caminho, "w"), ensure_ascii=False, indent=2); open(caminho, "a").write("\n")
print("== PASSOU" if ok else "== FALHOU", motor)
sys.exit(0 if ok else 1)
PY
