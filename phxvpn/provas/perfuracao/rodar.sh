#!/bin/bash
# Prova da perfuracao de NAT (UDP hole punching mediado pelo repasse).
#
# Topologia (tudo em `ip netns`, como root):
#
#   pfA 192.168.1.2 --- pfNA [NAT] 203.0.113.1 ---+
#                                                 +-- pfI (a "internet", ponte) -- pfR 203.0.113.10 (repasse)
#   pfB 192.168.2.2 --- pfNB [NAT] 203.0.113.2 ---+
#
# A e B nao tem rota um para o outro: so saem pelos NATs (MASQUERADE). Os
# NATs tem o firewall de todo roteador domestico: da internet, so entra o
# que responde a algo que saiu (ctstate NEW na wan -> DROP).
#
# Cenarios:
#   cone        -- MASQUERADE comum (mapeamento independente do destino);
#                  perfuracao LIGADA. Esperado: migra ao direto e o repasse
#                  nao carrega dados.
#   desligada   -- o mesmo NAT, com --sem-perfuracao. E o outro sentido da
#                  prova: a exigencia «dados nao passam pelo repasse» REPROVA.
#   simetrico   -- NAT de B com --random (porta nova por destino). Esperado:
#                  a perfuracao desiste e o ping segue pelo repasse.
#   bloqueado   -- NAT cone, mas o NAT de A descarta UDP entre os dois NATs.
#                  Esperado: idem ao simetrico.
#   desbloqueia -- comeca como o bloqueado; depois que a primeira rodada
#                  desiste, o bloqueio cai. Esperado: a tentativa em segundo
#                  plano (60 s depois) migra sozinha.
#   sem-firewall -- NAT cone SEM a regra de firewall na wan (hipotese medida:
#                  a sonda que chega antes do furo deixa rastro no conntrack).
#
# Uso: ./rodar.sh [cenario ...]   (sem argumento: todos). Grava
# resultados.json ao lado deste roteiro.
set -u
U=$(cd "$(dirname "$0")" && pwd)
BIN=${PHXVPN_BIN:-$U/../../target/debug/phxvpn}
D=${PHXVPN_PROVA_DIR:-/var/tmp/phx-perfuracao}
NS="pfI pfR pfNA pfNB pfA pfB"
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
  ip -n pfI link add br0 type bridge; ip -n pfI link set br0 up
  for par in pfR:203.0.113.10 pfNA:203.0.113.1 pfNB:203.0.113.2; do
    local n=${par%%:*} ip=${par#*:}
    ip link add i$n netns pfI type veth peer name wan netns $n
    ip -n pfI link set i$n master br0 up
    ip -n $n addr add $ip/24 dev wan; ip -n $n link set wan up
  done
  for par in A:1 B:2; do
    local q=${par%%:*} r=${par#*:}
    ip link add lan netns pfN$q type veth peer name eth0 netns pf$q
    ip -n pfN$q addr add 192.168.$r.1/24 dev lan; ip -n pfN$q link set lan up
    ip -n pf$q addr add 192.168.$r.2/24 dev eth0; ip -n pf$q link set eth0 up
    ip -n pf$q route add default via 192.168.$r.1
    ip netns exec pfN$q sysctl -qw net.ipv4.ip_forward=1
    local rnd=""
    [ "$c" = simetrico ] && [ $q = B ] && rnd="--random"
    ip netns exec pfN$q iptables -t nat -A POSTROUTING -o wan -j MASQUERADE $rnd
    if [ "$c" != sem-firewall ]; then
      ip netns exec pfN$q iptables -A INPUT -i wan -m conntrack --ctstate NEW -j DROP
      ip netns exec pfN$q iptables -A FORWARD -i wan -m conntrack --ctstate NEW -j DROP
    fi
  done
  if [ "$c" = bloqueado ] || [ "$c" = desbloqueia ]; then
    ip netns exec pfNA iptables -I FORWARD -o wan -p udp -d 203.0.113.2 -j DROP
    ip netns exec pfNA iptables -I INPUT -i wan -p udp -s 203.0.113.2 -j DROP
  fi
  # Contadores: no repasse, tudo e so os datagramas grandes (os de dados do
  # ping de 1000 bytes; REGISTRO e APRESENTAR tem 80); no NAT de A, o que
  # saiu DIRETO para o NAT de B.
  ip netns exec pfR iptables -A INPUT -p udp --dport 51821 -m length --length 300:65535
  ip netns exec pfR iptables -A INPUT -p udp --dport 51821
  ip netns exec pfNA iptables -I FORWARD -o wan -p udp -d 203.0.113.2
}

contador() { # ns cadeia linha -> "pacotes bytes"
  ip netns exec $1 iptables -nvxL $2 | awk -v l=$3 'NR==l+2 {print $1, $2}'
}

rodar() { # $1 = cenario
  local c=$1 extra=""
  [ "$c" = desligada ] && extra="--sem-perfuracao"
  montar $c
  local W=$D/$c; rm -rf $W; mkdir -p $W; cd $W
  local PR PA PB
  PR=$($BIN p2p chave --arquivo r.chave)
  PA=$($BIN p2p chave --arquivo a.chave)
  PB=$($BIN p2p chave --arquivo b.chave)
  ip netns exec pfR $BIN repasse --porta 51821 --chave r.chave > rep.log 2>&1 &
  sleep 0.3
  local t0; t0=$(date +%s.%N)
  ip netns exec pfA env PHXVPN_SENHA_REDE=senha-da-prova $BIN p2p ligar --rede prova \
    --ip 10.78.0.1/24 --chave a.chave --par $PB@10.78.0.2 --modo auto \
    --repasse $PR@203.0.113.10:51821 --interface phx0 $extra > a.log 2>&1 &
  ip netns exec pfB env PHXVPN_SENHA_REDE=senha-da-prova $BIN p2p ligar --rede prova \
    --ip 10.78.0.2/24 --chave b.chave --par $PA@10.78.0.1 --modo auto \
    --repasse $PR@203.0.113.10:51821 --interface phx0 $extra > b.log 2>&1 &
  # O relogio da migracao comeca quando os dois estao no ar (a derivacao da
  # senha da rede, PBKDF2 de 310.000 voltas, fica fora da conta).
  for _ in $(seq 600); do
    grep -q "no ar" a.log && grep -q "no ar" b.log && break; sleep 0.05
  done
  t0=$(date +%s.%N)
  # Espera: ou os dois furam, ou a rodada de sondas termina (pedido + 10
  # sondas de 1 s), com folga. O tunel sobe pelo repasse de qualquer jeito.
  local migrou=0 t_mig=null espera=250
  if [ $c = desbloqueia ]; then
    # A primeira rodada falha (sondas bloqueadas); o bloqueio cai e a rodada
    # seguinte -- `RETENTAR` = 60 s depois -- tem de migrar sozinha.
    for _ in $(seq 250); do grep -q "sem resposta" a.log b.log && break; sleep 0.1; done
    ip netns exec pfNA iptables -D FORWARD -o wan -p udp -d 203.0.113.2 -j DROP
    ip netns exec pfNA iptables -D INPUT -i wan -p udp -s 203.0.113.2 -j DROP
    espera=900
  fi
  for _ in $(seq $espera); do
    if grep -q perfurado a.log && grep -q perfurado b.log; then
      migrou=1; t_mig=$(echo "$(date +%s.%N) - $t0" | bc); break
    fi
    sleep 0.1
  done
  [ $migrou = 1 ] || sleep 1
  # Um ping de aquecimento acorda o que estiver dormindo; os contadores
  # zeram depois dele.
  ip netns exec pfA ping -c 2 -W 3 10.78.0.2 > /dev/null 2>&1
  ip netns exec pfR iptables -Z; ip netns exec pfNA iptables -Z
  local saida recebidos
  saida=$(ip netns exec pfA ping -c $PING_N -i 0.2 -W 2 -s 1000 10.78.0.2)
  recebidos=$(echo "$saida" | awk '/received/ {print $4}')
  local rtt; rtt=$(echo "$saida" | awk -F'/' '/^rtt/ {print $5}')
  read -r rep_dados_p rep_dados_b <<< "$(contador pfR INPUT 1)"
  read -r rep_tudo_p rep_tudo_b <<< "$(contador pfR INPUT 2)"
  read -r dir_p dir_b <<< "$(contador pfNA FORWARD 1)"
  # Evidencia do mapeamento: o que o conntrack de cada NAT guarda entre os
  # dois enderecos publicos (sport= da linha de volta e a porta que o NAT deu).
  for q in A B; do
    ip netns exec pfN$q cat /proc/net/nf_conntrack 2>/dev/null | grep udp \
      | grep -E "203\.0\.113\.(1|2) .*203\.0\.113\.(1|2) " > conntrack-N$q.txt
  done
  local sem_resposta=0; grep -q "sem resposta" a.log b.log && sem_resposta=1
  # O criterio da prova: ping inteiro E nenhum datagrama de dados no repasse
  # E trafego saindo direto de um NAT para o outro.
  local fora=false
  [ "${recebidos:-0}" = $PING_N ] && [ "$rep_dados_p" = 0 ] && [ "$dir_p" -gt 0 ] && fora=true
  # O que cada cenario tem de dar: so o cone passa no criterio; os outros
  # tem de REPROVAR nele e ainda assim pingar tudo (nunca pior que hoje).
  local esperado=false; [ $c = cone ] || [ $c = desbloqueia ] && esperado=true
  local confere=false
  [ $fora = $esperado ] && [ "${recebidos:-0}" = $PING_N ] && confere=true
  [ $confere = true ] || FALHOU=1
  echo "== $c: criterio_dados_fora_do_repasse=$fora (esperado $esperado) -> $([ $confere = true ] && echo CONFERE || echo NAO-CONFERE)"
  echo "   migrou=$migrou (${t_mig}s) ping=$recebidos/$PING_N rtt=${rtt}ms repasse_dados=${rep_dados_p}p/${rep_dados_b}B repasse_total=${rep_tudo_p}p/${rep_tudo_b}B direto_A->NB=${dir_p}p/${dir_b}B"
  sed 's/^/   a| /' a.log | grep -v "^   a| $" | tail -4; sed 's/^/   b| /' b.log | tail -4
  RES+=("{\"cenario\":\"$c\",\"migrou_para_direto\":$([ $migrou = 1 ] && echo true || echo false),\"segundos_ate_migrar\":$t_mig,\"ping_enviados\":$PING_N,\"ping_recebidos\":${recebidos:-0},\"rtt_medio_ms\":${rtt:-null},\"repasse_pacotes_de_dados\":$rep_dados_p,\"repasse_bytes_de_dados\":$rep_dados_b,\"repasse_pacotes_total\":$rep_tudo_p,\"repasse_bytes_total\":$rep_tudo_b,\"direto_pacotes_A_para_NB\":$dir_p,\"direto_bytes_A_para_NB\":$dir_b,\"desistiu_da_perfuracao\":$([ $sem_resposta = 1 ] && echo true || echo false),\"criterio_dados_fora_do_repasse\":$fora,\"esperado\":$esperado,\"confere\":$confere}")
  limpar
  cd $U
}

[ -x "$BIN" ] || { echo "sem binario: $BIN (cargo build)"; exit 1; }
mkdir -p $D
CEN=("$@"); [ ${#CEN[@]} = 0 ] && CEN=(cone desligada simetrico bloqueado desbloqueia sem-firewall)
RES=(); FALHOU=0
for c in "${CEN[@]}"; do rodar $c; done
{
  echo "{"
  echo "  \"prova\": \"perfuracao de NAT mediada pelo repasse\","
  echo "  \"medido_em\": \"$(date -Is)\","
  echo "  \"kernel\": \"$(uname -r)\","
  echo "  \"ping\": \"$PING_N x 1000 bytes, intervalo 0,2 s, contadores zerados depois de 2 pings de aquecimento\","
  echo "  \"cenarios\": ["
  for i in "${!RES[@]}"; do
    sep=","; [ $i = $((${#RES[@]} - 1)) ] && sep=""
    echo "    ${RES[$i]}$sep"
  done
  echo "  ]"
  echo "}"
} > $U/resultados.json
echo "gravado: $U/resultados.json"
[ $FALHOU = 0 ] && echo "PROVA: todos os cenarios conferem" || { echo "PROVA: cenario que NAO confere (veja acima)"; exit 1; }
