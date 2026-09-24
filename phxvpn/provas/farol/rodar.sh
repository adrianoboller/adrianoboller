#!/bin/bash
# Prova do FAROL: um membro alcancavel da propria rede faz o papel do
# repasse, SEM nenhum `phxvpn repasse` em lugar nenhum.
#
# Topologia (tudo em `ip netns`, como root):
#
#   frA  203.0.113.10 (IP publico: o farol, e o dono da rede) ---+
#   frA2 203.0.113.11 (segundo farol, so no cenario dois-farois) -+
#   frX  203.0.113.20 (fora do rol) -----------------------------+-- frI (ponte)
#   frB 192.168.2.2 -- frNB [NAT] 203.0.113.2 -------------------+
#   frC 192.168.3.2 -- frNC [NAT] 203.0.113.3 -------------------+
#
# B e C nao tem rota entre si. Os NATs tem o firewall de roteador domestico
# (da wan so entra resposta). Cenarios:
#   cone          -- NATs cone. Medido: B<->C direto em ~1 s pela LISTA DE
#                    PARES que A ensina (os dois se mandam INICIO juntos); o
#                    farol nao carrega dado. E o mesmo sem farol (sem-farol-cone).
#   cone-libera   -- cone com o direto B<->C bloqueado no NAT de B ate o tunel
#                    subir pelo farol e a 1a rodada de sondas falhar; entao
#                    libera. Esperado: a rodada seguinte, APRESENTADA PELO
#                    FAROL (tipos 10/11), fura e o farol para de carregar dado.
#   simetrico    -- NAT de C com --random. Esperado: B<->C pelo RELE do farol;
#                    tcpdump em A so ve cifrado; X (fora do rol) nao usa A.
#   sem-farol     -- o mesmo simetrico, sem marcar o farol. Esperado: B e C
#                    NAO se falam (o outro sentido da prova).
#   sem-farol-cone -- o cone sem farol: medido, nao esperado (ver PHXVPN.md).
#   dois-farois   -- A e A2 farois, NAT simetrico; o farol que carrega cai.
#                    Esperado: B<->C voltam pelo outro; mede a interrupcao.
#
# Uso: ./rodar.sh [cenario ...]. Grava resultados.json ao lado.
set -u
U=$(cd "$(dirname "$0")" && pwd)
BIN=${PHXVPN_BIN:-$U/../../target/release/phxvpn}
D=${PHXVPN_PROVA_DIR:-/var/tmp/phx-farol}
NS="frI frA frA2 frX frNB frNC frB frC"
SENHA=senha-da-prova-farol
PADRAO=c0ffee1234567890deadbeefcafe4242
PING_N=20

limpar() {
  for n in $NS; do
    # Mata por PID, dentro do namespace -- nunca por nome de processo.
    for p in $(ip netns pids $n 2>/dev/null); do kill "$p" 2>/dev/null; done
  done
  sleep 0.5
  for n in $NS; do ip netns del $n 2>/dev/null; done
}

montar() { # $1 = cenario
  local c=$1
  limpar
  for n in $NS; do ip netns add $n; ip -n $n link set lo up; done
  ip -n frI link add br0 type bridge; ip -n frI link set br0 up
  for par in frA:203.0.113.10 frA2:203.0.113.11 frX:203.0.113.20 frNB:203.0.113.2 frNC:203.0.113.3; do
    local n=${par%%:*} ip=${par#*:}
    ip link add i$n netns frI type veth peer name wan netns $n
    ip -n frI link set i$n master br0 up
    ip -n $n addr add $ip/24 dev wan; ip -n $n link set wan up
  done
  for par in B:2 C:3; do
    local q=${par%%:*} r=${par#*:}
    ip link add lan netns frN$q type veth peer name eth0 netns fr$q
    ip -n frN$q addr add 192.168.$r.1/24 dev lan; ip -n frN$q link set lan up
    ip -n fr$q addr add 192.168.$r.2/24 dev eth0; ip -n fr$q link set eth0 up
    ip -n fr$q route add default via 192.168.$r.1
    ip netns exec frN$q sysctl -qw net.ipv4.ip_forward=1
    local rnd=""
    case "$c" in simetrico|sem-farol|dois-farois) [ $q = C ] && rnd="--random";; esac
    ip netns exec frN$q iptables -t nat -A POSTROUTING -o wan -j MASQUERADE $rnd
    ip netns exec frN$q iptables -A INPUT -i wan -m conntrack --ctstate NEW -j DROP
    ip netns exec frN$q iptables -A FORWARD -i wan -m conntrack --ctstate NEW -j DROP
  done
  # Contadores: no farol, os datagramas grandes que chegam (dados de B para C
  # embrulhados; REGISTRO/APRESENTAR tem 80 B); no NAT de B, o que sai direto
  # para o NAT de C.
  for f in frA frA2; do
    ip netns exec $f iptables -A INPUT -p udp --dport 51820 -m length --length 300:65535
  done
  ip netns exec frNB iptables -I FORWARD -o wan -p udp -d 203.0.113.3
  if [ "$c" = cone-libera ]; then
    # Depois do contador (linha 1 continua sendo o contador do direto).
    ip netns exec frNB iptables -A FORWARD -o wan -p udp -d 203.0.113.3 -j DROP
    ip netns exec frNB iptables -A FORWARD -i wan -p udp -s 203.0.113.3 -j DROP
  fi
}

contador() { # ns cadeia linha -> "pacotes bytes"
  ip netns exec $1 iptables -nvxL $2 | awk -v l=$3 'NR==l+2 {print $1, $2}'
}

no() { # ns dir comando... (com a senha da rede no ambiente)
  local ns=$1 dir=$2; shift 2
  ip netns exec $ns env PHXVPN_SENHA_REDE=$SENHA sh -c "cd $dir && exec $BIN $*"
}

esperar_log() { # arquivo texto decimos
  for _ in $(seq $3); do grep -q "$2" $1 2>/dev/null && return 0; sleep 0.1; done
  return 1
}

rodar() { # $1 = cenario
  local c=$1
  montar $c
  local W=$D/$c; rm -rf $W; mkdir -p $W/a $W/a2 $W/b $W/c $W/x; cd $W
  local PA PX
  PA=$(cd a && $BIN p2p chave --arquivo p2p.chave)
  (cd a2 && $BIN p2p chave --arquivo p2p.chave > /dev/null)
  (cd b && $BIN p2p chave --arquivo p2p.chave > /dev/null)
  (cd c && $BIN p2p chave --arquivo p2p.chave > /dev/null)
  PX=$(cd x && $BIN p2p chave --arquivo p2p.chave)
  (cd a && $BIN p2p criar --rede prova --ip 10.78.0.1/24 --modo auto --sem-descoberta > criar.log)
  local farol=false
  case $c in cone|cone-libera|simetrico|dois-farois) farol=true;; esac
  if [ $farol = true ]; then
    (cd a && $BIN p2p farol --rede prova --endereco 203.0.113.10:51820 > farol.log)
  fi
  convite() { (cd a && PHXVPN_SENHA_REDE=$SENHA $BIN p2p convidar --rede prova --endereco 203.0.113.10:51820); }
  no frA $W/a p2p ligar --rede prova --interface phx0 > $W/a.log 2>&1 &
  esperar_log $W/a.log "no ar" 300
  if [ $c = dois-farois ]; then
    # A2 entra primeiro; o dono o marca farol com a rede ligada (o no do
    # dono rele o rol do disco em 2 s e o espalha).
    local k2; k2=$(convite)
    (cd a2 && PHXVPN_SENHA_REDE=$SENHA $BIN p2p entrar $k2 --sem-descoberta > entrar.log)
    no frA2 $W/a2 p2p ligar --rede prova --interface phx0 --farol > $W/a2.log 2>&1 &
    esperar_log $W/a2.log "no ar" 300
    for _ in $(seq 100); do ip netns exec frA2 ping -c1 -W1 10.78.0.1 > /dev/null 2>&1 && break; done
    local ipa2; ipa2=$(grep -o 'seu IP [0-9.]*' a2/entrar.log | awk '{print $3}')
    (cd a && $BIN p2p farol --rede prova --ip $ipa2 --endereco 203.0.113.11:51820 >> farol.log)
    esperar_log $W/a2.log "farol ligado" 100
  fi
  local kb kc
  kb=$(convite); kc=$(convite)
  (cd b && PHXVPN_SENHA_REDE=$SENHA $BIN p2p entrar $kb --sem-descoberta > entrar.log)
  (cd c && PHXVPN_SENHA_REDE=$SENHA $BIN p2p entrar $kc --sem-descoberta > entrar.log)
  local ipb ipc
  ipb=$(grep -o 'seu IP [0-9.]*' b/entrar.log | awk '{print $3}')
  ipc=$(grep -o 'seu IP [0-9.]*' c/entrar.log | awk '{print $3}')
  no frB $W/b p2p ligar --rede prova --interface phx0 > $W/b.log 2>&1 &
  no frC $W/c p2p ligar --rede prova --interface phx0 > $W/c.log 2>&1 &
  esperar_log $W/b.log "no ar" 300; esperar_log $W/c.log "no ar" 300
  local t0; t0=$(date +%s.%N)
  # Captura no farol A durante tudo o que vem depois.
  ip netns exec frA tcpdump -i wan -w $W/farol.pcap -U udp port 51820 > /dev/null 2>&1 &
  local tcpd=$!
  # Tempo ate o primeiro ping B -> C (prazo 60 s).
  local t_prim=null
  for _ in $(seq 60); do
    if ip netns exec frB ping -c1 -W1 $ipc > /dev/null 2>&1; then
      t_prim=$(echo "$(date +%s.%N) - $t0" | bc); break
    fi
  done
  # Da tempo a perfuracao (2 s quando passa; 10 sondas quando nao).
  local t_perf=null
  if [ $c = cone-libera ]; then
    # O direto esteve bloqueado ate aqui: a lista de pares nao furou, o
    # tunel subiu pelo farol e a 1a rodada de sondas falhou. Libera-se o
    # direto; a rodada seguinte (60 s depois), APRESENTADA PELO FAROL, fura.
    esperar_log $W/b.log "sem resposta" 300
    ip netns exec frNB iptables -D FORWARD -o wan -p udp -d 203.0.113.3 -j DROP
    ip netns exec frNB iptables -D FORWARD -i wan -p udp -s 203.0.113.3 -j DROP
    for _ in $(seq 1000); do
      if grep -q "caminho direto (perfurado)" b.log && grep -q "caminho direto (perfurado)" c.log; then
        t_perf=$(echo "$(date +%s.%N) - $t0" | bc); break
      fi
      sleep 0.1
    done
    sleep 2
  elif [ "$t_prim" != null ]; then
    sleep 14
  fi
  for f in frA frA2; do ip netns exec $f iptables -Z; done
  ip netns exec frNB iptables -Z
  local saida recebidos rtt
  saida=$(ip netns exec frB ping -c $PING_N -i 0.2 -W 2 -s 1000 -p $PADRAO $ipc)
  recebidos=$(echo "$saida" | awk '/received/ {print $4}')
  rtt=$(echo "$saida" | awk -F'/' '/^rtt/ {print $5}')
  local fa_p fa_b fa2_p fa2_b dir_p dir_b
  read -r fa_p fa_b <<< "$(contador frA INPUT 1)"
  read -r fa2_p fa2_b <<< "$(contador frA2 INPUT 1)"
  read -r dir_p dir_b <<< "$(contador frNB FORWARD 1)"
  # Fora do rol: X sabe a senha da rede e aponta A como repasse.
  local x_recebidos=null
  if [ $c = simetrico ]; then
    local PC; PC=$(cd c && $BIN p2p chave --arquivo p2p.chave)
    no frX $W/x p2p ligar --rede prova --ip 10.78.0.9/24 --interface phx0 --par $PC@$ipc \
      --modo repasse --repasse $PA@203.0.113.10:51820 --sem-descoberta > $W/x.log 2>&1 &
    esperar_log $W/x.log "no ar" 300
    x_recebidos=$(ip netns exec frX ping -c 5 -i 1 -W 2 $ipc | awk '/received/ {print $4}')
  fi
  # Dois farois: derruba o que carrega e mede a interrupcao.
  local caiu=null volta=null maior_vao=null perdidos=null
  if [ $c = dois-farois ]; then
    local ns_cai=frA; [ "${fa2_p:-0}" -gt "${fa_p:-0}" ] && ns_cai=frA2
    caiu=$ns_cai
    ip netns exec frB ping -D -i 0.2 -W 1 -c 250 $ipc > $W/ping-queda.txt 2>&1 &
    local pp=$!
    sleep 4
    date +%s.%N > $W/t-queda.txt
    for p in $(ip netns pids $ns_cai); do kill "$p"; done
    wait $pp
    read -r maior_vao volta perdidos <<< "$(python3 - $W/ping-queda.txt $W/t-queda.txt <<'PY'
import re,sys
linhas=open(sys.argv[1]).read()
t=[float(m) for m in re.findall(r'^\[(\d+\.\d+)\].*bytes from', linhas, re.M)]
queda=float(open(sys.argv[2]).read())
depois=[x for x in t if x>queda]
vaos=[b-a for a,b in zip(t,t[1:])]
m=re.search(r'(\d+) packets transmitted, (\d+) received', linhas)
perd=int(m.group(1))-int(m.group(2)) if m else -1
print(f"{max(vaos):.1f}" if vaos else "null", f"{depois[0]-queda:.1f}" if depois else "null", perd)
PY
)"
  fi
  kill $tcpd 2>/dev/null; wait $tcpd 2>/dev/null
  # O que o farol viu entre B e C: o padrao do ping e o cabecalho IP de
  # dentro (10.78.0.x -> 10.78.0.y) nao podem aparecer em claro.
  local claro
  claro=$(python3 - $W/farol.pcap $PADRAO $ipb $ipc <<'PY'
import sys,socket
d=open(sys.argv[1],'rb').read()
pad=bytes.fromhex(sys.argv[2])
cab=socket.inet_aton(sys.argv[3])+socket.inet_aton(sys.argv[4])
print(d.count(pad[:8])+d.count(cab))
PY
)
  local pcap_bytes; pcap_bytes=$(stat -c %s $W/farol.pcap 2>/dev/null || echo 0)
  # O caminho dos 20 pings, pelos contadores (nao pelo registro): direto do
  # NAT de B ao de C, ou pelo rele de um farol.
  local caminho=nenhum
  if [ "${dir_p:-0}" -gt 0 ]; then
    caminho=direto
    grep -q "perfurado" b.log c.log && caminho=direto-perfurado-pelo-farol
  elif [ "${fa_p:-0}" -gt 0 ] || [ "${fa2_p:-0}" -gt 0 ]; then
    caminho=rele-do-farol
  fi
  local recusou=0
  recusou=$(grep -o '[0-9]* recusados' a.log | tail -1 | awk '{print $1}')
  echo "== $c: primeiro ping ${t_prim}s, ping $recebidos/$PING_N rtt ${rtt}ms, caminho $caminho"
  echo "   farol A ${fa_p}p/${fa_b}B  farol A2 ${fa2_p}p/${fa2_b}B  direto NB->NC ${dir_p}p/${dir_b}B  claro no farol: $claro (pcap $pcap_bytes B)"
  [ $c = simetrico ] && echo "   X fora do rol: ping $x_recebidos/5, A recusou ${recusou:-0}"
  [ $c = dois-farois ] && echo "   caiu $caiu: primeira resposta depois ${volta}s, maior vao ${maior_vao}s, perdidos $perdidos"
  for f in a a2 b c x; do [ -s $f.log ] && sed "s/^/   $f| /" $f.log | grep -E "farol|perfur|caminho" | tail -3; done
  python3 - <<PY >> $D/res.jsonl
import json
def n(x):
    try: return float(x) if '.' in str(x) else int(x)
    except Exception: return None
print(json.dumps({"cenario":"$c","farol_marcado":"$farol"=="true",
  "segundos_ate_primeiro_ping":n("$t_prim"),"segundos_ate_perfurar_pelo_farol":n("$t_perf"),"ping_enviados":$PING_N,"ping_recebidos":n("${recebidos:-0}"),
  "rtt_medio_ms":n("${rtt:-}"),"caminho":"$caminho",
  "farol_A_pacotes_de_dados":n("${fa_p:-0}"),"farol_A_bytes_de_dados":n("${fa_b:-0}"),
  "farol_A2_pacotes_de_dados":n("${fa2_p:-0}"),"farol_A2_bytes_de_dados":n("${fa2_b:-0}"),
  "direto_pacotes_NB_para_NC":n("${dir_p:-0}"),"direto_bytes_NB_para_NC":n("${dir_b:-0}"),
  "pcap_no_farol_bytes":n("$pcap_bytes"),"texto_claro_no_pcap_do_farol":n("$claro"),
  "fora_do_rol_ping_recebidos":n("$x_recebidos"),"farol_recusados":n("${recusou:-0}"),
  "farol_que_caiu":None if "$caiu"=="null" else "$caiu","segundos_ate_voltar_depois_da_queda":n("$volta"),
  "maior_vao_do_ping_s":n("$maior_vao"),"pings_perdidos_na_queda":n("$perdidos")}, ensure_ascii=False))
PY
  limpar
  cd $U
}

[ -x "$BIN" ] || { echo "sem binario: $BIN (cargo build --release)"; exit 1; }
mkdir -p $D; rm -f $D/res.jsonl
CEN=("$@"); [ ${#CEN[@]} = 0 ] && CEN=(cone cone-libera simetrico sem-farol sem-farol-cone dois-farois)
for c in "${CEN[@]}"; do rodar $c; done
python3 - $D/res.jsonl $U/resultados.json "$(uname -r)" <<'PY'
import json,sys,datetime
res=[json.loads(l) for l in open(sys.argv[1])]
esperado={
 # No cone o direto nasce da LISTA DE PARES que A ensina (medido: o mesmo
 # sem farol) -- o farol nao carrega dado nenhum ali.
 "cone": lambda r: r["ping_recebidos"]==20 and r["caminho"].startswith("direto")
         and r["farol_A_pacotes_de_dados"]==0 and r["texto_claro_no_pcap_do_farol"]==0,
 "cone-libera": lambda r: r["ping_recebidos"]==20 and r["caminho"]=="direto-perfurado-pelo-farol"
         and r["farol_A_pacotes_de_dados"]==0 and r["segundos_ate_perfurar_pelo_farol"] is not None,
 "simetrico": lambda r: r["ping_recebidos"]==20 and r["farol_A_pacotes_de_dados"]>=40 and r["direto_pacotes_NB_para_NC"]==0
              and r["texto_claro_no_pcap_do_farol"]==0 and r["fora_do_rol_ping_recebidos"]==0,
 "sem-farol": lambda r: r["ping_recebidos"]==0 and r["segundos_ate_primeiro_ping"] is None,
 "sem-farol-cone": lambda r: True,
 "dois-farois": lambda r: r["ping_recebidos"]==20 and r["segundos_ate_voltar_depois_da_queda"] is not None,
}
for r in res:
    r["confere"]=esperado[r["cenario"]](r)
out={"prova":"farol: membro alcancavel faz o papel do repasse, sem phxvpn repasse",
     "medido_em":datetime.datetime.now(datetime.timezone.utc).isoformat(timespec="seconds"),
     "kernel":sys.argv[3],
     "ping":"20 x 1000 bytes com padrao, intervalo 0,2 s, contadores zerados 14 s depois do primeiro ping",
     "cenarios":res}
json.dump(out,open(sys.argv[2],"w"),ensure_ascii=False,indent=1)
print("gravado:",sys.argv[2])
falhou=[r["cenario"] for r in res if not r["confere"]]
print("PROVA:", "todos os cenarios conferem" if not falhou else "NAO confere: "+", ".join(falhou))
sys.exit(1 if falhou else 0)
PY
