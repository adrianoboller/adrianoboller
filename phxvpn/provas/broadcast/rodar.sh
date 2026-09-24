#!/bin/bash
# Prova da DIFUSAO no P2P: broadcast e multicast da placa de um membro chegam
# aos outros (`src/difusao.rs`).
#
# Topologia (tudo em `ip netns`, como root): tres nos na mesma ponte, sem NAT
# -- o que se prova aqui e a replicacao, nao a travessia.
#
#   bcA 203.0.113.10 (dono, 10.78.0.1) --+
#   bcB 203.0.113.11 ---------------------+-- bcI (ponte)
#   bcC 203.0.113.12 ---------------------+
#
# Cenarios:
#   ligada      -- A manda 20 a cada destino (10.78.0.255, 255.255.255.255,
#                  239.255.255.250, 224.0.0.251): B e C contam 20 de cada.
#                  B manda 20 a 255.255.255.255: A e C contam. Laco: os
#                  contadores rx das placas ficam parados na janela quieta.
#                  Rajada: 1000 pacotes de 64 B e 1000 de 1300 B, sem pausa:
#                  so o teto passa. SSDP: um M-SEARCH de A acha B e C.
#   desligada-a -- A com --sem-difusao: B e C contam 0 (o unicast segue).
#   desligada-b -- B com --sem-difusao: B conta 0, C conta tudo.
#
# Uso: ./rodar.sh [cenario ...]. Grava resultados.json ao lado.
set -u
U=$(cd "$(dirname "$0")" && pwd)
BIN=${PHXVPN_BIN:-$U/../../target/release/phxvpn}
D=${PHXVPN_PROVA_DIR:-/var/tmp/phx-difusao}
NS="bcI bcA bcB bcC"
SENHA=senha-da-prova-difusao
PY="python3 $U/udp.py"
DESTINOS="10.78.0.255 255.255.255.255 239.255.255.250 224.0.0.251"
GRUPOS="239.255.255.250 224.0.0.251"

limpar() {
  for n in $NS; do
    # Mata por PID, dentro do namespace -- nunca por nome de processo.
    for p in $(ip netns pids $n 2>/dev/null); do kill "$p" 2>/dev/null; done
  done
  sleep 0.5
  for n in $NS; do ip netns del $n 2>/dev/null; done
}

montar() {
  limpar
  for n in $NS; do ip netns add $n; ip -n $n link set lo up; done
  ip -n bcI link add br0 type bridge; ip -n bcI link set br0 up
  for par in bcA:203.0.113.10 bcB:203.0.113.11 bcC:203.0.113.12; do
    local n=${par%%:*} ip=${par#*:}
    ip link add i$n netns bcI type veth peer name wan netns $n
    ip -n bcI link set i$n master br0 up
    ip -n $n addr add $ip/24 dev wan; ip -n $n link set wan up
  done
}

no() { # ns dir comando...
  local ns=$1 dir=$2; shift 2
  ip netns exec $ns env PHXVPN_SENHA_REDE=$SENHA sh -c "cd $dir && exec $BIN $*"
}

esperar_log() { # arquivo texto decimos
  for _ in $(seq $3); do grep -q "$2" $1 2>/dev/null && return 0; sleep 0.1; done
  return 1
}

rx() { ip netns exec $1 cat /sys/class/net/phx0/statistics/rx_packets; }
# Datagramas que o kernel jogou fora por soquete UDP cheio (o do no).
rcvbuf() { ip netns exec $1 awk '/^Udp:/ {n++; if (n==1) for (i=1;i<=NF;i++) c[$i]=i; else print $c["RcvbufErrors"]}' /proc/net/snmp; }

soma_json() { # json chave -> numero (0 se ausente)
  python3 -c "import json,sys; print(json.loads(sys.argv[1] or '{}').get(sys.argv[2],0))" "$1" "$2"
}

rodar() { # $1 = cenario
  local c=$1
  montar
  local W=$D/$c; rm -rf $W; mkdir -p $W/a $W/b $W/c; cd $W
  (cd a && $BIN p2p chave --arquivo p2p.chave > /dev/null)
  (cd b && $BIN p2p chave --arquivo p2p.chave > /dev/null)
  (cd c && $BIN p2p chave --arquivo p2p.chave > /dev/null)
  (cd a && $BIN p2p criar --rede prova --ip 10.78.0.1/24 --modo direto --sem-descoberta > criar.log)
  convite() { (cd a && PHXVPN_SENHA_REDE=$SENHA $BIN p2p convidar --rede prova --endereco 203.0.113.10:51820); }
  local kb kc; kb=$(convite); kc=$(convite)
  (cd b && PHXVPN_SENHA_REDE=$SENHA $BIN p2p entrar $kb --sem-descoberta > entrar.log)
  (cd c && PHXVPN_SENHA_REDE=$SENHA $BIN p2p entrar $kc --sem-descoberta > entrar.log)
  local ipb ipc
  ipb=$(grep -o 'seu IP [0-9.]*' b/entrar.log | awk '{print $3}')
  ipc=$(grep -o 'seu IP [0-9.]*' c/entrar.log | awk '{print $3}')
  local fa="" fb=""
  [ $c = desligada-a ] && fa=--sem-difusao
  [ $c = desligada-b ] && fb=--sem-difusao
  no bcA $W/a p2p ligar --rede prova --interface phx0 $fa > $W/a.log 2>&1 &
  esperar_log $W/a.log "no ar" 300
  no bcB $W/b p2p ligar --rede prova --interface phx0 $fb > $W/b.log 2>&1 &
  no bcC $W/c p2p ligar --rede prova --interface phx0 > $W/c.log 2>&1 &
  esperar_log $W/b.log "no ar" 300; esperar_log $W/c.log "no ar" 300
  # Malha inteira de pe: A-B, A-C e B-C (este pela lista de pares).
  local malha=false
  for _ in $(seq 60); do
    if ip netns exec bcB ping -c1 -W1 10.78.0.1 > /dev/null 2>&1 \
      && ip netns exec bcC ping -c1 -W1 10.78.0.1 > /dev/null 2>&1 \
      && ip netns exec bcB ping -c1 -W1 $ipc > /dev/null 2>&1; then
      malha=true; break
    fi
    sleep 0.5
  done

  # 1. Cada destino, 20 vezes, de A; B e C ouvem.
  local seg=$(( $(echo $DESTINOS | wc -w) + 3 ))
  ip netns exec bcB $PY ouvir 9999 $ipb $seg $GRUPOS > $W/b.cont &
  local pb=$!
  ip netns exec bcC $PY ouvir 9999 $ipc $seg $GRUPOS > $W/c.cont &
  local pc=$!
  sleep 0.5
  local rxa0 rxb0 rxc0; rxa0=$(rx bcA); rxb0=$(rx bcB); rxc0=$(rx bcC)
  for d in $DESTINOS; do
    ip netns exec bcA $PY mandar $d 9999 20 0.05 10.78.0.1 > /dev/null
  done
  wait $pb $pc
  # 2. De B, ao broadcast limitado: A e C ouvem (qualquer no origina).
  ip netns exec bcA $PY ouvir 9999 10.78.0.1 3 > $W/a.cont2 &
  local pa2=$!
  ip netns exec bcC $PY ouvir 9999 $ipc 3 > $W/c.cont2 &
  local pc2=$!
  sleep 0.5
  ip netns exec bcB $PY mandar 255.255.255.255 9999 20 0.05 $ipb > /dev/null
  wait $pa2 $pc2
  # 3. Laco: janela quieta de 5 s; o rx das placas nao anda.
  local rxa1 rxb1 rxc1; rxa1=$(rx bcA); rxb1=$(rx bcB); rxc1=$(rx bcC)
  sleep 5
  local rxa2 rxb2 rxc2; rxa2=$(rx bcA); rxb2=$(rx bcB); rxc2=$(rx bcC)

  # 4. Rajadas (so no cenario ligada): 1000 sem pausa, pequenos e grandes.
  local raj_p='None' raj_g='None' ssdp='None' rbb0=0 rbb1=0 rbc0=0 rbc1=0
  if [ $c = ligada ]; then
    rbb0=$(rcvbuf bcB); rbc0=$(rcvbuf bcC)
    for tam in 64 1300; do
      sleep 2 # o balde enche de novo
      ip netns exec bcB $PY ouvir 9999 $ipb 4 > $W/b.raj$tam &
      local p1=$!
      ip netns exec bcC $PY ouvir 9999 $ipc 4 > $W/c.raj$tam &
      local p2=$!
      sleep 0.5
      ip netns exec bcA $PY mandar 10.78.0.255 9999 1000 0 10.78.0.1 $tam > $W/a.raj$tam
      wait $p1 $p2
    done
    rbb1=$(rcvbuf bcB); rbc1=$(rcvbuf bcC)
    raj_p=$(python3 -c "import json;print(json.dumps({'enviados':1000,'tamanho':64,'segundos':json.load(open('$W/a.raj64'))['segundos'],'b':json.load(open('$W/b.raj64')).get('10.78.0.255',0),'c':json.load(open('$W/c.raj64')).get('10.78.0.255',0)}))")
    raj_g=$(python3 -c "import json;print(json.dumps({'enviados':1000,'tamanho':1300,'segundos':json.load(open('$W/a.raj1300'))['segundos'],'b':json.load(open('$W/b.raj1300')).get('10.78.0.255',0),'c':json.load(open('$W/c.raj1300')).get('10.78.0.255',0)}))")
    # 5. SSDP de verdade no formato: M-SEARCH em multicast, resposta unicast.
    sleep 2
    ip netns exec bcB $PY ssdp-responder $ipb 4 > $W/b.ssdp &
    local s1=$!
    ip netns exec bcC $PY ssdp-responder $ipc 4 > $W/c.ssdp &
    local s2=$!
    sleep 0.5
    ssdp=$(ip netns exec bcA $PY ssdp-buscar 10.78.0.1 3)
    wait $s1 $s2
  fi
  local uni=false
  ip netns exec bcB ping -c1 -W1 10.78.0.1 > /dev/null 2>&1 && uni=true
  local ca cb cc
  ca=$(grep -c "acima do teto" $W/a.log); cb=$(grep -c "acima do teto" $W/b.log); cc=$(grep -c "acima do teto" $W/c.log)
  limpar

  python3 - <<PY >> $D/res.jsonl
import json
def ler(f):
    try: return json.load(open(f))
    except Exception: return {}
b, c = ler("$W/b.cont"), ler("$W/c.cont")
print(json.dumps({
  "cenario": "$c", "malha_de_pe": "$malha" == "true", "unicast_depois": "$uni" == "true",
  "de_a_por_destino_20_cada": {d: {"b": b.get(d, 0), "c": c.get(d, 0)} for d in "$DESTINOS".split()},
  "de_b_255_20": {"a": ler("$W/a.cont2").get("255.255.255.255", 0), "c": ler("$W/c.cont2").get("255.255.255.255", 0)},
  "rx_placa_no_envio": {"a": $rxa1 - $rxa0, "b": $rxb1 - $rxb0, "c": $rxc1 - $rxc0},
  "rx_placa_janela_quieta_5s": {"a": $rxa2 - $rxa1, "b": $rxb2 - $rxb1, "c": $rxc2 - $rxc1},
  "rajada_64B": $raj_p, "rajada_1300B": $raj_g, "ssdp": $ssdp,
  "avisos_de_corte_no_log": {"a": $ca, "b": $cb, "c": $cc},
  "udp_rcvbuf_errors_nas_rajadas": {"b": $rbb1 - $rbb0, "c": $rbc1 - $rbc0},
}))
PY
  echo "== $c: $(tail -1 $D/res.jsonl)"
}

[ $# -eq 0 ] && set -- ligada desligada-a desligada-b
mkdir -p $D; rm -f $D/res.jsonl
for c in "$@"; do rodar $c; done
python3 - $D/res.jsonl $U/resultados.json "$(uname -r)" $U/../../src/difusao.rs <<'PY'
import json,re,sys,datetime
res=[json.loads(l) for l in open(sys.argv[1])]
# Os tetos saem do fonte, nao de numero digitado aqui.
fonte=open(sys.argv[4]).read()
def const(nome):
    return eval(re.search(r"const %s: \w+ = ([^;]+);" % nome, fonte).group(1).replace("_",""))
tetos={"pacotes_por_s":const("TETO_PACOTES_POR_S"),"bytes_por_s":const("TETO_BYTES_POR_S"),
       "tamanho":"MTU da placa (TETO_TAMANHO)"}
out={"prova":"difusao P2P (broadcast/multicast) em netns",
     "data":datetime.datetime.now().strftime("%Y-%m-%d %H:%M"),
     "kernel":sys.argv[3],
     "tetos":tetos,
     "n":"uma corrida por cenario",
     "cenarios":res}
json.dump(out,open(sys.argv[2],"w"),ensure_ascii=False,indent=1)
PY
echo "gravado $U/resultados.json"
