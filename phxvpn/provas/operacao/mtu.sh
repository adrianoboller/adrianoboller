#!/bin/bash
# Prova do MTU do P2P (item 6 das lacunas): o maior ping com DF que atravessa
# o tunel, pelo caminho DIRETO, pelo REPASSE e pelo FAROL, com os fragmentos
# IP descartados no caminho -- o que CGNAT e firewall de operadora costumam
# fazer. Conta tambem os fragmentos que o fio produziu com o descarte
# desligado, e a vazao TCP do direto (iperf3) para decidir se baixar o MTU
# custa algo medivel.
#
# Topologias (tudo em `ip netns`, como root; mata por PID, nunca por nome):
#   direto/repasse: mtA 203.0.113.1, mtB 203.0.113.2, mtR 203.0.113.10 (repasse)
#                   na ponte mtI. `--modo direto` ou `--modo repasse`.
#   farol:          a do provas/farol «simetrico": mtA farol publico; mtB e mtC
#                   atras de NAT (o de C com --random); B<->C so pelo rele do farol.
#
# Descarte de fragmento: nft em prerouting com prioridade -450, ANTES do
# defrag do conntrack (-400) -- depois dele o fragmento ja virou pacote
# inteiro e a regra nao o veria.
#
# Uso: PHXVPN_BIN=... ./mtu.sh ROTULO   (grava a secao mtu-ROTULO)
set -u
U=$(cd "$(dirname "$0")" && pwd)
BIN=${PHXVPN_BIN:-$U/../../target/release/phxvpn}
ROTULO=${1:-novo}
D=${PHXVPN_PROVA_DIR:-/var/tmp/phx-mtu}
NS="mtI mtA mtB mtC mtR mtNB mtNC"
SENHA=senha-da-prova-mtu

limpar() {
  for n in $NS; do for p in $(ip netns pids $n 2>/dev/null); do kill "$p" 2>/dev/null; done; done
  sleep 0.5
  for n in $NS; do ip netns del $n 2>/dev/null; done
}
trap limpar EXIT

ponte() { # ns:ip ...
  ip netns add mtI; ip -n mtI link set lo up
  ip -n mtI link add br0 type bridge; ip -n mtI link set br0 up
  for par in "$@"; do
    local n=${par%%:*} ip=${par#*:}
    ip netns add $n 2>/dev/null; ip -n $n link set lo up
    ip link add i$n netns mtI type veth peer name wan netns $n
    ip -n mtI link set i$n master br0 up
    ip -n $n addr add $ip/24 dev wan; ip -n $n link set wan up
  done
}

fragmentos() { # liga (drop=1) ou so conta (drop=0) em todos os ns
  local drop=$1 acao=""; [ "$drop" = 1 ] && acao=drop
  for n in $NS; do
    [ -e /var/run/netns/$n ] || continue
    ip netns exec $n nft delete table ip mtufrag 2>/dev/null
    ip netns exec $n nft -f - <<EOF
table ip mtufrag {
  chain pre { type filter hook prerouting priority -450; ip frag-off & 0x3fff != 0 counter $acao; }
}
EOF
  done
}
contar_fragmentos() {
  local t=0
  for n in $NS; do
    [ -e /var/run/netns/$n ] || continue
    local c; c=$(ip netns exec $n nft list table ip mtufrag 2>/dev/null | grep -o 'packets [0-9]*' | awk '{s+=$2} END {print s+0}')
    t=$((t + c))
  done
  echo $t
}

maior_ping() { # ns destino -> maior -s com resposta (-M do), busca binaria
  local ns=$1 dst=$2 lo=0 hi=1472
  ip netns exec $ns ping -c1 -W1 -M do -s $lo $dst >/dev/null 2>&1 || { echo null; return; }
  while [ $((hi - lo)) -gt 1 ]; do
    local m=$(((lo + hi) / 2))
    if ip netns exec $ns ping -c2 -i 0.2 -W1 -M do -s $m $dst 2>/dev/null | grep -q " 0% packet loss\| 50% packet loss"; then lo=$m; else hi=$m; fi
  done
  echo $lo
}

no() { local ns=$1 dir=$2; shift 2; ip netns exec $ns env PHXVPN_SENHA_REDE=$SENHA sh -c "cd $dir && exec $BIN $*"; }
esperar_log() { for _ in $(seq $3); do grep -q "$2" $1 2>/dev/null && return 0; sleep 0.1; done; return 1; }

medir() { # cenario origem destino -> linha JSON
  local c=$1 ns=$2 dst=$3
  for _ in $(seq 100); do ip netns exec $ns ping -c1 -W1 $dst >/dev/null 2>&1 && break; done
  local mtu_placa; mtu_placa=$(ip -n $ns link show phx0 | grep -o 'mtu [0-9]*' | awk '{print $2}')
  fragmentos 0
  ip netns exec $ns ping -c 5 -i 0.2 -W1 -M do -s $((mtu_placa - 28)) $dst >/dev/null 2>&1
  local frag_cheio; frag_cheio=$(contar_fragmentos)
  local cheio_sem_descarte; cheio_sem_descarte=$(ip netns exec $ns ping -c 5 -i 0.2 -W1 -M do -s $((mtu_placa - 28)) $dst 2>/dev/null | awk '/received/ {print $4}')
  fragmentos 1
  local cheio_com_descarte; cheio_com_descarte=$(ip netns exec $ns ping -c 5 -i 0.2 -W1 -M do -s $((mtu_placa - 28)) $dst 2>/dev/null | awk '/received/ {print $4}')
  local maior; maior=$(maior_ping $ns $dst)
  # TCP pela placa com fragmento descartado: o MSS vem do MTU da placa, e o
  # segmento cheio e exatamente o que o fio nao leva.
  local tcp=null
  if [ "$c" != direto ]; then
    ip netns exec ${4:-mtB} timeout 20 iperf3 -s -1 >/dev/null 2>&1 &
    sleep 0.5
    tcp=$(ip netns exec $ns timeout 15 iperf3 -c $dst -t 3 -J 2>/dev/null | python3 -c "import json,sys
try: print(round(json.load(sys.stdin)['end']['sum_received']['bits_per_second']/1e6,1))
except Exception: print('null')")
    [ -z "$tcp" ] && tcp=null
  fi
  fragmentos 0
  python3 -c "import json; null=None; print(json.dumps({'cenario':'$c','mtu_placa':$mtu_placa,
 'ping_cheio':$((mtu_placa - 28)),'fragmentos_no_fio_com_5_pings_cheios':$frag_cheio,
 'pings_cheios_sem_descarte_de_fragmento':${cheio_sem_descarte:-0},
 'pings_cheios_com_fragmento_descartado':${cheio_com_descarte:-0},
 'maior_ping_df_com_fragmento_descartado':$maior,
 'tcp_mbit_com_fragmento_descartado':$tcp}))"
}

vazao_direta() { # ns destino -> 3 medidas de iperf3 TCP (Mbit/s)
  local ns=$1 dst=$2 out=""
  for i in 1 2 3; do
    ip netns exec mtB timeout 20 iperf3 -s -1 >/dev/null 2>&1 &
    sleep 0.5
    local v; v=$(ip netns exec $ns timeout 15 iperf3 -c $dst -t 4 -J 2>/dev/null | python3 -c "import json,sys
print(round(json.load(sys.stdin)['end']['sum_received']['bits_per_second']/1e6,1))")
    out="$out${out:+,}$v"
    sleep 0.5
  done
  echo "[$out]"
}

rodar_par() { # direto|repasse
  local c=$1
  limpar; ponte mtA:203.0.113.1 mtB:203.0.113.2 mtR:203.0.113.10
  local W=$D/$c; rm -rf $W; mkdir -p $W; cd $W
  local PR PA PB
  PR=$($BIN p2p chave --arquivo r.chave); PA=$($BIN p2p chave --arquivo a.chave); PB=$($BIN p2p chave --arquivo b.chave)
  local extra=""
  if [ $c = repasse ]; then
    ip netns exec mtR $BIN repasse --porta 51821 --chave r.chave > rep.log 2>&1 &
    sleep 0.3
    extra="--modo repasse --repasse $PR@203.0.113.10:51821 --sem-perfuracao"
  fi
  ip netns exec mtA env PHXVPN_SENHA_REDE=$SENHA $BIN p2p ligar --rede prova --ip 10.78.0.1/24 --chave a.chave \
    --par $PB@10.78.0.2@203.0.113.2:51820 --interface phx0 --sem-descoberta $extra > a.log 2>&1 &
  ip netns exec mtB env PHXVPN_SENHA_REDE=$SENHA $BIN p2p ligar --rede prova --ip 10.78.0.2/24 --chave b.chave \
    --par $PA@10.78.0.1@203.0.113.1:51820 --interface phx0 --sem-descoberta $extra > b.log 2>&1 &
  esperar_log a.log "no ar" 600; esperar_log b.log "no ar" 600
  local linha; linha=$(medir $c mtA 10.78.0.2 mtB)
  if [ $c = direto ]; then
    # O custo de baixar o MTU: a mesma vazao com a placa dos dois lados em
    # 1420 (o de antes) e em 1384 (o que o rele sobre IPv6 cabe), no mesmo
    # tunel -- so o `ip link set mtu` muda entre as duas.
    local v1420 v1384
    ip -n mtA link set phx0 mtu 1420; ip -n mtB link set phx0 mtu 1420
    v1420=$(vazao_direta mtA 10.78.0.2)
    ip -n mtA link set phx0 mtu 1384; ip -n mtB link set phx0 mtu 1384
    v1384=$(vazao_direta mtA 10.78.0.2)
    linha=$(python3 -c "import json,sys; d=json.loads(sys.argv[1]); d['tcp_mbit_direto_3x_placa_1420']=json.loads(sys.argv[2]); d['tcp_mbit_direto_3x_placa_1384']=json.loads(sys.argv[3]); print(json.dumps(d))" "$linha" "$v1420" "$v1384")
  fi
  echo "$linha"
  cd $U
}

rodar_farol() {
  limpar; ponte mtA:203.0.113.10 mtNB:203.0.113.2 mtNC:203.0.113.3
  for par in B:2 C:3; do
    local q=${par%%:*} r=${par#*:}
    ip netns add mt$q; ip -n mt$q link set lo up
    ip link add lan netns mtN$q type veth peer name eth0 netns mt$q
    ip -n mtN$q addr add 192.168.$r.1/24 dev lan; ip -n mtN$q link set lan up
    ip -n mt$q addr add 192.168.$r.2/24 dev eth0; ip -n mt$q link set eth0 up
    ip -n mt$q route add default via 192.168.$r.1
    ip netns exec mtN$q sysctl -qw net.ipv4.ip_forward=1
    local rnd=""; [ $q = C ] && rnd="--random"
    ip netns exec mtN$q iptables -t nat -A POSTROUTING -o wan -j MASQUERADE $rnd
    ip netns exec mtN$q iptables -A INPUT -i wan -m conntrack --ctstate NEW -j DROP
    ip netns exec mtN$q iptables -A FORWARD -i wan -m conntrack --ctstate NEW -j DROP
  done
  local W=$D/farol; rm -rf $W; mkdir -p $W/a $W/b $W/c; cd $W
  (cd a && $BIN p2p chave --arquivo p2p.chave >/dev/null)
  (cd b && $BIN p2p chave --arquivo p2p.chave >/dev/null)
  (cd c && $BIN p2p chave --arquivo p2p.chave >/dev/null)
  (cd a && $BIN p2p criar --rede prova --ip 10.78.0.1/24 --modo auto --sem-descoberta > criar.log)
  (cd a && $BIN p2p farol --rede prova --endereco 203.0.113.10:51820 > farol.log)
  convite() { (cd a && PHXVPN_SENHA_REDE=$SENHA $BIN p2p convidar --rede prova --endereco 203.0.113.10:51820); }
  no mtA $W/a p2p ligar --rede prova --interface phx0 > $W/a.log 2>&1 &
  esperar_log $W/a.log "no ar" 300
  local kb kc; kb=$(convite); kc=$(convite)
  (cd b && PHXVPN_SENHA_REDE=$SENHA $BIN p2p entrar $kb --sem-descoberta > entrar.log)
  (cd c && PHXVPN_SENHA_REDE=$SENHA $BIN p2p entrar $kc --sem-descoberta > entrar.log)
  local ipc; ipc=$(grep -o 'seu IP [0-9.]*' c/entrar.log | awk '{print $3}')
  no mtB $W/b p2p ligar --rede prova --interface phx0 > $W/b.log 2>&1 &
  no mtC $W/c p2p ligar --rede prova --interface phx0 > $W/c.log 2>&1 &
  esperar_log $W/b.log "no ar" 300; esperar_log $W/c.log "no ar" 300
  # O caminho tem de ser o rele do farol: o NAT de C e simetrico.
  for _ in $(seq 60); do ip netns exec mtB ping -c1 -W1 $ipc >/dev/null 2>&1 && break; done
  sleep 14
  medir farol mtB $ipc mtC
  cd $U
}

[ -x "$BIN" ] || { echo "sem binario: $BIN"; exit 1; }
mkdir -p $D
R=$D/res-$ROTULO.jsonl; : > $R
CEN=${PHXVPN_CENARIOS:-"direto repasse farol"}
for c in $CEN; do
  if [ $c = farol ]; then rodar_farol | tee -a $R; else rodar_par $c | tee -a $R; fi
done
python3 - $R "$BIN" "$(uname -r)" > $D/secao-$ROTULO.json <<'PY'
import json, sys, datetime
res = [json.loads(l) for l in open(sys.argv[1]) if l.strip()]
print(json.dumps({
  "medido_em": datetime.datetime.now(datetime.timezone.utc).isoformat(timespec="seconds"),
  "binario": sys.argv[2], "kernel": sys.argv[3], "n": 1,
  "metodo": "ping -M do pela placa; busca binaria do maior -s com resposta; fragmentos IP descartados por nft prerouting -450 em todos os netns; iperf3 TCP 3 s",
  "cenarios": res}, ensure_ascii=False, indent=1))
PY
python3 $U/juntar.py mtu-$ROTULO $D/secao-$ROTULO.json
