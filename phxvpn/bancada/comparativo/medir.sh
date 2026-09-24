#!/bin/bash
# Bancada comparativa: phxvpn x OpenVPN x WireGuard, MESMA topologia (dois
# netns por veth), MESMO trabalho (iperf3 TCP 5 s pelo tunel, ping de 20).
# Todos em espaco de usuario: o kernel daqui nao tem o modulo do WireGuard
# nem o DCO do OpenVPN -- dito no resultado, nao escondido.
# Uso (root): ./medir.sh [corridas=3]   -> resultados.json ao lado
set -u
AQUI=$(cd "$(dirname "$0")" && pwd); R=$(cd "$AQUI/../.." && pwd)
PHX=$R/target/release/phxvpn; N=${1:-3}; T=$(mktemp -d /tmp/phx-comp.XXXX)
ns() { ip netns exec "$@"; }
subir_rede() {
  for n in cA cB; do ip netns del $n 2>/dev/null; ip netns add $n; ip -n $n link set lo up; done
  ip link add ca0 netns cA type veth peer name cb0 netns cB
  ip -n cA addr add 192.168.60.1/24 dev ca0; ip -n cA link set ca0 up
  ip -n cB addr add 192.168.60.2/24 dev cb0; ip -n cB link set cb0 up
}
derrubar() { for n in cA cB; do ip netns pids $n 2>/dev/null | xargs -r kill 2>/dev/null; done; sleep 0.5; for n in cA cB; do ip netns del $n 2>/dev/null; done; }
# conectar: espera o primeiro ping passar pelo tunel; imprime segundos.
conectar() { local ip=$1 t0=$(date +%s.%N)
  for _ in $(seq 300); do ns cB ping -c1 -W1 "$ip" >/dev/null 2>&1 && { python3 -c "print(round($(date +%s.%N)-$t0,2))"; return; }; sleep 0.1; done; echo -1; }
medir() { # medir NOME IP_DO_OUTRO_LADO
  local nome=$1 ip=$2
  local con; con=$(conectar "$ip")
  ns cA iperf3 -s -D -B "$ip" >/dev/null 2>&1; sleep 0.5
  local mbits=()
  for _ in $(seq $N); do
    mbits+=("$(ns cB iperf3 -c "$ip" -t 5 -J 2>/dev/null | python3 -c "import json,sys;print(round(json.load(sys.stdin)['end']['sum_received']['bits_per_second']/1e6,1))" 2>/dev/null || echo 0)")
  done
  local rtt; rtt=$(ns cB ping -c 20 -i 0.2 -q "$ip" | awk -F/ '/rtt/{print $5}')
  echo "{\"nome\":\"$nome\",\"conectar_s\":$con,\"mbits\":[$(IFS=,; echo "${mbits[*]}")],\"rtt_ms\":${rtt:-0}}"
}
RES=()

# --- phxvpn P2P (Noise IKpsk2, ChaCha20-Poly1305)
subir_rede
(cd $T && $PHX p2p chave --arquivo $T/a.chave >/dev/null && $PHX p2p chave --arquivo $T/b.chave >/dev/null)
PA=$($PHX p2p chave --arquivo $T/a.chave | grep -o '[0-9a-f]\{64\}'); PB=$($PHX p2p chave --arquivo $T/b.chave | grep -o '[0-9a-f]\{64\}')
ns cA env PHXVPN_SENHA_REDE=senha-bancada $PHX p2p ligar --rede B --ip 10.78.0.1/24 --chave $T/a.chave --interface phx0 --par "$PB@10.78.0.2@192.168.60.2:51820" > $T/pa.log 2>&1 &
ns cB env PHXVPN_SENHA_REDE=senha-bancada $PHX p2p ligar --rede B --ip 10.78.0.2/24 --chave $T/b.chave --interface phx0 --par "$PA@10.78.0.1@192.168.60.1:51820" > $T/pb.log 2>&1 &
sleep 1; RES+=("$(medir phxvpn 10.78.0.1)"); derrubar

# --- WireGuard (wireguard-go, ChaCha20-Poly1305)
subir_rede
wg genkey > $T/wa; wg genkey > $T/wb; WA=$(wg pubkey < $T/wa); WB=$(wg pubkey < $T/wb)
for q in A:1:$T/wa:$WB:2 B:2:$T/wb:$WA:1; do IFS=: read -r L I K P O <<< "$q"
  # Nome por lado: o `wg` acha a interface por /var/run/wireguard/<nome>.sock,
  # e esse caminho e o MESMO nos dois netns -- com os dois chamados wg0, um
  # atropelava o soquete do outro (achado na primeira corrida).
  ns c$L env WG_PROCESS_FOREGROUND=1 wireguard-go wg$L > $T/wg$L.log 2>&1 & sleep 0.7
  ns c$L wg set wg$L private-key $K listen-port 51821 peer $P endpoint 192.168.60.$O:51821 allowed-ips 10.79.0.$O/32
  ns c$L ip addr add 10.79.0.$I/24 dev wg$L; ns c$L ip link set wg$L mtu 1420 up
done
RES+=("$(medir wireguard-go 10.79.0.1)"); derrubar

# --- OpenVPN 2.6 (TLS 1.3, certificado; duas cifras de dados)
openssl req -x509 -newkey ed25519 -nodes -keyout $T/ca.key -out $T/ca.crt -days 2 -subj /CN=ca >/dev/null 2>&1
for q in a b; do
  openssl req -newkey ed25519 -nodes -keyout $T/$q.key -out $T/$q.csr -subj /CN=$q >/dev/null 2>&1
  openssl x509 -req -in $T/$q.csr -CA $T/ca.crt -CAkey $T/ca.key -CAcreateserial -out $T/$q.crt -days 2 >/dev/null 2>&1
done
for cifra in AES-256-GCM CHACHA20-POLY1305; do
  subir_rede
  comum="--dev tun0 --proto udp --port 1194 --ca $T/ca.crt --dh none --tls-version-min 1.3 --data-ciphers $cifra --verb 1"
  ns cA openvpn $comum --tls-server --cert $T/a.crt --key $T/a.key --ifconfig 10.80.0.1 10.80.0.2 > $T/oa.log 2>&1 &
  ns cB openvpn $comum --tls-client --cert $T/b.crt --key $T/b.key --ifconfig 10.80.0.2 10.80.0.1 --remote 192.168.60.1 > $T/ob.log 2>&1 &
  RES+=("$(medir "openvpn-$cifra" 10.80.0.1)"); derrubar
done

printf '{"data":"%s","corridas":%s,"maquina":"%s","resultados":[%s]}\n' "$(date -u +%FT%TZ)" $N "$(nproc) nucleos, $(uname -r)" "$(IFS=,; echo "${RES[*]}")" > $AQUI/resultados.json
python3 - "$AQUI/resultados.json" <<'PY'
import json,sys,statistics as st
j=json.load(open(sys.argv[1]))
print(f"{'':28} {'conectar':>9} {'Mbit/s mín–med–máx':>24} {'ping':>8}")
for r in j['resultados']:
    m=r['mbits']; print(f"{r['nome']:28} {r['conectar_s']:>8}s {min(m):>7}–{st.median(m):>7}–{max(m):>7} {r['rtt_ms']:>6} ms")
PY
rm -rf $T
