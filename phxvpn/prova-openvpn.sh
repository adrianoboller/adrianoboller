#!/usr/bin/env bash
# Prova do modo servidor com o OpenVPN DE VERDADE, de ponta a ponta, numa
# maquina Linux so (root): PostgreSQL descartavel -> painel -> instalar ->
# criar rede -> dois membros em netns com o `openvpn` -> ping entre eles ->
# remover um -> a CRL o barra na reconexao. Depois, o caso TCP: rede com
# `proto tcp-server` na 443, UDP bloqueado nos dois membros, a ana so
# alcanca o servidor por um proxy HTTP (CONNECT) -- com o `http-proxy` no
# perfil passa; sem ele, nao.
#
# Uso:  sudo ./prova-openvpn.sh [pasta-de-trabalho]
# Pede: openvpn 2.6+, postgresql 16 (initdb), python3, ip netns.
set -euo pipefail

AQUI=$(cd "$(dirname "$0")" && pwd)
T=${1:-$(mktemp -d /tmp/phxvpn-prova-ovpn.XXXX)}
BIN=${PHXVPN_BIN:-$AQUI/target/debug/phxvpn}
PGBIN=$(ls -d /usr/lib/postgresql/*/bin | sort -V | tail -1)
PGPORTA=55433
PAINEL=127.0.0.1:8471
mkdir -p "$T"; chmod 755 "$T"
echo "== pasta: $T"

limpar() {
  set +e
  for n in pxc1 pxc2 pxp; do ip netns pids $n 2>/dev/null | xargs -r kill; ip netns del $n 2>/dev/null; rm -rf /etc/netns/$n; done
  [ -n "${PID_PAINEL:-}" ] && kill "$PID_PAINEL" 2>/dev/null
  pkill -x openvpn 2>/dev/null
  su postgres -c "$PGBIN/pg_ctl -D $T/pg -m fast stop" >/dev/null 2>&1
  ip link del pxbr 2>/dev/null
}
trap limpar EXIT

# --- PostgreSQL descartavel, com SCRAM
mkdir -p "$T/pg"; chown postgres "$T/pg"
echo "senha-pg-prova" > "$T/pgsenha"; chown postgres "$T/pgsenha"
su postgres -c "$PGBIN/initdb -D $T/pg -A scram-sha-256 --pwfile=$T/pgsenha -U postgres" >/dev/null
su postgres -c "$PGBIN/pg_ctl -D $T/pg -o '-p $PGPORTA -k /tmp -c listen_addresses=127.0.0.1' -l $T/pg/pg.log start" >/dev/null
sleep 2

# --- Rede de mentira: ponte no host, dois clientes em netns
ip link add pxbr type bridge; ip addr add 192.168.88.1/24 dev pxbr; ip link set pxbr up
for i in 1 2; do
  ip netns add pxc$i; ip -n pxc$i link set lo up
  mkdir -p /etc/netns/pxc$i; echo "192.168.88.1 vpn.prova.local" > /etc/netns/pxc$i/hosts
  ip link add pxv$i type veth peer name eth0 netns pxc$i
  ip link set pxv$i master pxbr up
  ip -n pxc$i addr add 192.168.88.1$i/24 dev eth0; ip -n pxc$i link set eth0 up
done

# --- Painel com o supervisor do OpenVPN
PHXVPN_PG="host=127.0.0.1 port=$PGPORTA user=postgres password=senha-pg-prova dbname=postgres" \
  "$BIN" painel --dados "$T/dados" --openvpn --escutar $PAINEL >"$T/painel.log" 2>&1 &
PID_PAINEL=$!
for _ in $(seq 50); do grep -q "CODIGO DE INSTALACAO" "$T/painel.log" && break; sleep 0.2; done
CODIGO=$(grep -o "CODIGO DE INSTALACAO: [^ ]*" "$T/painel.log" | awk '{print $4}')

api() { # api METODO CAMINHO TOKEN JSON
  python3 - "$@" <<'PY'
import json, sys, urllib.request
m, c, tk, corpo = sys.argv[1:5]
r = urllib.request.Request("http://127.0.0.1:8471" + c, method=m,
    data=corpo.encode() if m == "POST" else None,
    headers={"Content-Type": "application/json", **({"Authorization": "Bearer " + tk} if tk else {})})
try:
    print(urllib.request.urlopen(r).read().decode())
except urllib.error.HTTPError as e:
    print(e.read().decode()); sys.exit(1)
PY
}
campo() { python3 -c "import json,sys; print(json.load(sys.stdin)['$1'])"; }

api POST /api/instalar "" "{\"codigo_instalacao\":\"$CODIGO\",\"empresa\":\"Prova Ltda\",\"finalidade\":\"prova\",
 \"responsavel\":\"Prova\",\"email\":\"p@prova.local\",\"telefone\":\"0\",\"admin_usuario\":\"admin\",
 \"admin_senha\":\"senha-admin-longa\",\"senha_mestre\":\"senha-mestre-longa-da-prova\",
 \"servidor_nome\":\"vpn.prova.local\",\"servidor_ip\":\"192.168.88.1\",\"servidor_dns\":\"vpn.prova.local\",\"certificado_pem\":\"\"}" >/dev/null
TK_ADMIN=$(api POST /api/login "" '{"usuario":"admin","senha":"senha-admin-longa"}' | campo token)
api POST /api/redes "$TK_ADMIN" '{"nome":"Matriz","senha":"senha-da-rede","finalidade":"prova"}' | campo perfil > "$T/admin.ovpn"
api POST /api/usuarios "$TK_ADMIN" '{"login":"ana","senha":"senha-da-ana-longa","email":"ana@prova.local"}' >/dev/null
TK_ANA=$(api POST /api/login "" '{"usuario":"ana","senha":"senha-da-ana-longa"}' | campo token)
api POST /api/redes/entrar "$TK_ANA" '{"nome":"Matriz","senha":"senha-da-rede"}' | campo perfil > "$T/ana.ovpn"
echo "== perfis: $(wc -c < "$T/admin.ovpn") e $(wc -c < "$T/ana.ovpn") bytes"
sleep 1
pgrep -a openvpn | sed 's/^/== servidor: /'

# O perfil chama o servidor pelo nome; o nome resolve pelo hosts do netns.
sobe() { # sobe NETNS PERFIL LOG
  ip netns exec "$1" openvpn --config "$2" --log "$3" --daemon --writepid "$3.pid"
}
sobe pxc1 "$T/admin.ovpn" "$T/c1.log"; sobe pxc2 "$T/ana.ovpn" "$T/c2.log"
for _ in $(seq 60); do
  grep -q "Initialization Sequence Completed" "$T/c1.log" 2>/dev/null &&
  grep -q "Initialization Sequence Completed" "$T/c2.log" 2>/dev/null && break; sleep 0.5
done
IP1=$(ip -n pxc1 -4 -o addr show tun0 | awk '{print $4}' | cut -d/ -f1)
IP2=$(ip -n pxc2 -4 -o addr show tun0 | awk '{print $4}' | cut -d/ -f1)
GW=${IP1%.*}.1
echo "== admin=$IP1 ana=$IP2 servidor=$GW"
grep -h "TLSv1.3\|Data Channel: cipher" "$T/c1.log" | tail -2
ip netns exec pxc2 ping -c 3 -W 2 "$GW" | tail -2
echo "== ana -> admin (client-to-client):"
ip netns exec pxc2 ping -c 3 -W 2 "$IP1" | tail -2

echo "== remover a ana e reconectar"
REDE_ID=$(api GET /api/redes "$TK_ADMIN" "" | python3 -c "import json,sys; print(json.load(sys.stdin)[0]['id'])")
api POST /api/redes/remover "$TK_ADMIN" "{\"rede_id\":$REDE_ID,\"login\":\"ana\"}"
kill "$(cat "$T/c2.log.pid")"; sleep 1; : > "$T/c2b.log"
sobe pxc2 "$T/ana.ovpn" "$T/c2b.log"; sleep 8
if grep -q "Initialization Sequence Completed" "$T/c2b.log"; then
  echo "FALHOU: a ana removida conectou de novo"; exit 1
fi
LOGSRV=$(ls "$T"/dados/redes/*/openvpn.log | head -1)
# Rede com tls-crypt-v2: barrada ANTES do TLS pela serie; com v1, pela CRL.
if grep -q "tls-crypt-v2" "$T/ana.ovpn"; then
  grep -q "TLS CRYPT V2 VERIFY SCRIPT OK" "$LOGSRV" || { echo "FALHOU: v2 nao aceitou os membros"; exit 1; }
  grep -q "TLS CRYPT V2 VERIFY SCRIPT ERROR" "$LOGSRV" || { echo "FALHOU: v2 nao barrou a removida"; exit 1; }
  grep -h "VERIFY SCRIPT ERROR" "$LOGSRV" | tail -1
else
  grep -q "certificate revoked" "$LOGSRV" || { echo "FALHOU: a recusa nao foi pela CRL"; exit 1; }
fi
grep -q "UID set to \(phxvpn-ovpn\|nobody\)" "$LOGSRV" || { echo "FALHOU: o openvpn ficou como root"; exit 1; }
grep -h "UID set" "$LOGSRV" | tail -1
echo "== a ana removida NAO reconectou; o admin segue no ar:"
ip netns exec pxc1 ping -c 2 -W 2 "$GW" | tail -1

# ------------------------------------------------------------- TCP ----
echo "== TCP: rede Hotel em proto tcp-server, porta 443"
kill "$(cat "$T/c2b.log.pid")" 2>/dev/null
# O proxy mora num netns proprio, que resolve o nome do servidor.
ip netns add pxp; ip -n pxp link set lo up
mkdir -p /etc/netns/pxp; echo "192.168.88.1 vpn.prova.local" > /etc/netns/pxp/hosts
ip link add pxv3 type veth peer name eth0 netns pxp
ip link set pxv3 master pxbr up
ip -n pxp addr add 192.168.88.13/24 dev eth0; ip -n pxp link set eth0 up
ip netns exec pxp python3 "$AQUI/provas/tcp/proxy.py" 3128 "" "$T/proxy.log" &
# UDP bloqueado nos dois; a ana tambem nao alcanca a 443 direto: so o proxy.
for n in pxc1 pxc2; do ip netns exec $n iptables -A OUTPUT -p udp -j DROP; done
ip netns exec pxc2 iptables -A OUTPUT -p tcp -d 192.168.88.1 --dport 443 -j DROP
api POST /api/redes "$TK_ADMIN" '{"nome":"Hotel","senha":"senha-da-rede","finalidade":"prova","protocolo":"tcp","porta":443}' | campo perfil > "$T/admin-tcp.ovpn"
api POST /api/redes/entrar "$TK_ANA" '{"nome":"Hotel","senha":"senha-da-rede","http_proxy":"192.168.88.13:3128"}' | campo perfil > "$T/ana-tcp.ovpn"
# Usuario comum nao escolhe porta.
if api POST /api/redes "$TK_ANA" '{"nome":"Porta","senha":"senha-da-rede","protocolo":"tcp","porta":8443}' >/dev/null; then
  echo "FALHOU: usuario comum escolheu a porta"; exit 1
fi
sleep 1
CONFTCP=$(grep -l "^port 443" "$T"/dados/redes/*/servidor.conf)
grep -q "^proto tcp-server" "$CONFTCP" || { echo "FALHOU: servidor sem proto tcp-server"; exit 1; }
grep -h "^proto\|^remote\|^http-proxy" "$T/ana-tcp.ovpn" | sed 's/^/== perfil da ana: /'
ss -ltnp | grep -q ":443 .*openvpn" || { echo "FALHOU: ninguem escuta TCP 443"; exit 1; }
# Sentido 1: o mesmo perfil SEM o http-proxy nao passa (443 barrada).
grep -v "^http-proxy" "$T/ana-tcp.ovpn" > "$T/ana-tcp-sem-proxy.ovpn"
sobe pxc2 "$T/ana-tcp-sem-proxy.ovpn" "$T/c2t0.log"; sleep 10
if grep -q "Initialization Sequence Completed" "$T/c2t0.log"; then
  echo "FALHOU: sem proxy a ana conectou -- o bloqueio nao vale"; exit 1
fi
kill "$(cat "$T/c2t0.log.pid")"; sleep 1
echo "== sem proxy: nao conectou (esperado)"
# Sentido 2: com o proxy, passa.
sobe pxc1 "$T/admin-tcp.ovpn" "$T/c1t.log"; sobe pxc2 "$T/ana-tcp.ovpn" "$T/c2t.log"
for _ in $(seq 60); do
  grep -q "Initialization Sequence Completed" "$T/c1t.log" 2>/dev/null &&
  grep -q "Initialization Sequence Completed" "$T/c2t.log" 2>/dev/null && break; sleep 0.5
done
grep -q "Initialization Sequence Completed" "$T/c2t.log" || { echo "FALHOU: ana nao conectou pelo proxy"; tail -5 "$T/c2t.log"; exit 1; }
IP1T=$(ip -n pxc1 -4 -o addr show | awk '/10\.77\.2\./{print $4}' | cut -d/ -f1)
IP2T=$(ip -n pxc2 -4 -o addr show | awk '/10\.77\.2\./{print $4}' | cut -d/ -f1)
echo "== TCP: admin=$IP1T ana=$IP2T"
grep -h "TCP connection established\|Attempting to establish TCP" "$T/c2t.log" | tail -1
grep "CONNECT" "$T/proxy.log" | tail -1 | sed 's/^/== proxy: /'
grep -q "CONNECT vpn.prova.local:443 200" "$T/proxy.log" || { echo "FALHOU: a ana nao passou pelo proxy"; exit 1; }
echo "== ana -> admin pela rede TCP, via proxy:"
ip netns exec pxc2 ping -c 5 -W 2 "$IP1T" | tail -2
ip netns exec pxc2 ping -c 5 -W 2 "$IP1T" >/dev/null || { echo "FALHOU: ping pela rede TCP"; exit 1; }
echo "== PROVA OK"
