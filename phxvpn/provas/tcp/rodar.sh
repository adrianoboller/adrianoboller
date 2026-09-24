#!/bin/bash
# Prova do fio TCP ate o repasse (frente F2): rede que so deixa TCP, e rede
# que so deixa o proxy HTTP. Quatro netns numa ponte: repasse (ftrp,
# 10.0.9.1), nos A (ftpa, .2) e B (ftpb, .3) e o proxy (ftpx, .4). A e B nao
# se alcancam direto (iptables): tudo passa pelo repasse.
#
# Casos (UDP BLOQUEADO no repasse por iptables em todos os de 1 a 6):
#   1 auto (padrao)       -> passa pelo TCP depois da reserva de 10 s
#   2 --fio udp           -> NAO passa (o outro sentido da prova)
#   3 --tcp               -> passa
#   4 so o proxy alcanca o repasse; --tcp direto -> NAO passa (controle)
#   5 so o proxy alcanca; --proxy com usuario e senha -> passa
#   6 so o proxy alcanca; --proxy sem credencial -> NAO passa (407)
# Vazao (iperf3 5 s pelo tunel, N corridas; UDP liberado no caso UDP):
#   repasse por UDP x repasse por TCP x repasse por TCP via proxy.
#
# Uso (root): ./rodar.sh [corridas=5]  -> resultados.json ao lado
set -u
AQUI=$(cd "$(dirname "$0")" && pwd); R=$(cd "$AQUI/../.." && pwd)
PHX=${PHXVPN_BIN:-$R/target/release/phxvpn}; N=${1:-5}
T=$(mktemp -d /tmp/phx-tcp.XXXX)
SENHA_PROXY=senha-do-proxy-prova-7431
ns() { ip netns exec "$@"; }
NS="ftrp ftpa ftpb ftpx"

subir_rede() {
  ip link add ftbr type bridge; ip link set ftbr up
  local i=1
  for n in $NS; do
    ip netns add $n; ip -n $n link set lo up
    ip link add ftv$i type veth peer name eth0 netns $n
    ip link set ftv$i master ftbr up
    ip -n $n addr add 10.0.9.$i/24 dev eth0; ip -n $n link set eth0 up
    i=$((i + 1))
  done
  # A e B sem caminho direto: so o repasse (e o proxy) os ligam.
  ns ftpa iptables -A OUTPUT -d 10.0.9.3 -j DROP
  ns ftpb iptables -A OUTPUT -d 10.0.9.2 -j DROP
}
derrubar() {
  for n in $NS; do ip netns pids $n 2>/dev/null | xargs -r kill 2>/dev/null; done
  sleep 0.5
  for n in $NS; do ip netns del $n 2>/dev/null; done
  ip link del ftbr 2>/dev/null
}
trap 'derrubar; rm -rf $T' EXIT

# Regras do repasse. udp: bloqueia so o UDP. proxy: alem disso, o TCP 443
# so aceita quem vem do proxy. livre: nada.
regras() {
  ns ftrp iptables -F INPUT
  case $1 in
    udp)   ns ftrp iptables -A INPUT -p udp -j DROP ;;
    proxy) ns ftrp iptables -A INPUT -p udp -j DROP
           ns ftrp iptables -A INPUT -p tcp --dport 443 ! -s 10.0.9.4 -j DROP ;;
    livre) ;;
  esac
}

parar_nos() {
  for n in ftpa ftpb; do ip netns pids $n 2>/dev/null | xargs -r kill 2>/dev/null; done
  for _ in $(seq 50); do
    [ -z "$(ip netns pids ftpa)$(ip netns pids ftpb)" ] && break; sleep 0.1
  done
}

# ligar_nos ROTULO OPCOES... : sobe A e B com as mesmas opcoes de fio.
ligar_nos() {
  local rot=$1; shift
  ns ftpa env PHXVPN_SENHA_REDE=senha-da-rede PHXVPN_SENHA_PROXY=$SENHA_PROXY $PHX p2p ligar \
    --rede P --ip 10.78.0.1/24 --chave $T/a.chave --interface phx0 --par "$PB@10.78.0.2" \
    --modo repasse --repasse "$PR@10.0.9.1:51821" --repasse-tcp 443 "$@" > $T/$rot-a.log 2>&1 &
  ns ftpb env PHXVPN_SENHA_REDE=senha-da-rede PHXVPN_SENHA_PROXY=$SENHA_PROXY $PHX p2p ligar \
    --rede P --ip 10.78.0.2/24 --chave $T/b.chave --interface phx0 --par "$PA@10.78.0.1" \
    --modo repasse --repasse "$PR@10.0.9.1:51821" --repasse-tcp 443 "$@" > $T/$rot-b.log 2>&1 &
}

# Segundos ate o primeiro ping pelo tunel (-1 = nao passou no prazo).
primeiro_ping() { local prazo=$1 t0; t0=$(date +%s.%N)
  for _ in $(seq $((prazo * 5))); do
    ns ftpa ping -c1 -W1 10.78.0.2 >/dev/null 2>&1 && {
      python3 -c "print(round($(date +%s.%N)-$t0,1))"; return; }
    sleep 0.2
  done; echo -1; }

# caso ROTULO REGRAS PRAZO OPCOES...
CASOS=()
caso() {
  local rot=$1 reg=$2 prazo=$3; shift 3
  regras "$reg"; ligar_nos "$rot" "$@"; sleep 0.5
  local t; t=$(primeiro_ping "$prazo")
  local ok=0; [ "$t" != "-1" ] && ok=$(ns ftpa ping -c 5 -i 0.2 -W 2 10.78.0.2 2>/dev/null | grep -o '[0-9]* received' | cut -d' ' -f1)
  echo "== $rot ($reg, $*): primeiro ping ${t}s, $ok/5"
  CASOS+=("{\"caso\":\"$rot\",\"regras\":\"$reg\",\"opcoes\":\"$*\",\"primeiro_ping_s\":$t,\"pings_ok\":${ok:-0}}")
  parar_nos
}

subir_rede
$PHX p2p chave --arquivo $T/a.chave >/dev/null; $PHX p2p chave --arquivo $T/b.chave >/dev/null
PA=$($PHX p2p chave --arquivo $T/a.chave | grep -o '[0-9a-f]\{64\}')
PB=$($PHX p2p chave --arquivo $T/b.chave | grep -o '[0-9a-f]\{64\}')
ns ftrp $PHX repasse --porta 51821 --tcp 443 --chave $T/r.chave > $T/repasse.log 2>&1 &
for _ in $(seq 50); do grep -q "chave" $T/repasse.log && break; sleep 0.1; done
PR=$(grep -o 'chave [0-9a-f]\{64\}' $T/repasse.log | cut -d' ' -f2)
ns ftpx python3 $AQUI/proxy.py 3128 "prova:$SENHA_PROXY" $T/proxy.log &
ns ftpx python3 $AQUI/proxy.py 3129 "" $T/proxy-aberto.log &
sleep 0.5

caso auto          udp   25
caso so-udp        udp   25 --fio udp
caso tcp           udp   15 --tcp
caso tcp-sem-proxy proxy 15 --tcp
caso proxy         proxy 15 --proxy 10.0.9.4:3128 --proxy-usuario prova
# Mesmo proxy com senha, sem mandar credencial: 407 e nada passa.
caso proxy-sem-credencial proxy 12 --proxy 10.0.9.4:3128

grep -h "tentando TCP\|repasse por TCP\|407" $T/auto-a.log $T/proxy-sem-credencial-a.log | sort -u | sed 's/^/   /'
VAZOU=$(cat $T/*.log | grep -c "$SENHA_PROXY\|$(printf 'prova:%s' $SENHA_PROXY | base64)")
echo "== senha do proxy nos registros do phxvpn e do proxy: $VAZOU ocorrencia(s)"
CONNECTS=$(grep -c "CONNECT 10.0.9.1:443 200" $T/proxy.log)
C407=$(grep -c " 407" $T/proxy.log)

# --- Vazao: o mesmo iperf3 pelo tunel, fio por fio.
VAZAO=()
vazao() { # vazao ROTULO REGRAS OPCOES...
  local rot=$1 reg=$2; shift 2
  regras "$reg"; ligar_nos "vz-$rot" "$@"
  local t; t=$(primeiro_ping 25)
  ns ftpb iperf3 -s -D -B 10.78.0.2 >/dev/null 2>&1; sleep 0.5
  local m=()
  for _ in $(seq $N); do
    m+=("$(ns ftpa iperf3 -c 10.78.0.2 -t 5 -J 2>/dev/null | python3 -c "import json,sys;print(round(json.load(sys.stdin)['end']['sum_received']['bits_per_second']/1e6,1))" 2>/dev/null || echo 0)")
  done
  local rtt; rtt=$(ns ftpa ping -c 20 -i 0.2 -q 10.78.0.2 | awk -F/ '/rtt/{print $5}')
  echo "== vazao $rot: ${m[*]} Mbit/s, ping ${rtt} ms"
  VAZAO+=("{\"fio\":\"$rot\",\"primeiro_ping_s\":$t,\"mbits\":[$(IFS=,; echo "${m[*]}")],\"rtt_ms\":${rtt:-0}}")
  ip netns pids ftpb | xargs -r kill 2>/dev/null; parar_nos
}
vazao udp        livre --fio udp
vazao tcp        udp   --tcp
vazao tcp-proxy  proxy --proxy 10.0.9.4:3129

printf '{"data":"%s","corridas":%s,"maquina":"%s","casos":[%s],"senha_do_proxy_nos_registros":%s,"connects_200":%s,"respostas_407":%s,"vazao":[%s]}\n' \
  "$(date -u +%FT%TZ)" $N "$(nproc) nucleos, $(uname -r)" "$(IFS=,; echo "${CASOS[*]}")" \
  "$VAZOU" "$CONNECTS" "$C407" "$(IFS=,; echo "${VAZAO[*]}")" > $AQUI/resultados.json
python3 - "$AQUI/resultados.json" <<'PY'
import json, sys, statistics as st
j = json.load(open(sys.argv[1]))
print(f"{'fio':12} {'Mbit/s min-med-max':>26} {'ping':>9}")
for r in j['vazao']:
    m = r['mbits']
    print(f"{r['fio']:12} {min(m):>8}-{st.median(m):>7}-{max(m):>7} {r['rtt_ms']:>7} ms")
PY
