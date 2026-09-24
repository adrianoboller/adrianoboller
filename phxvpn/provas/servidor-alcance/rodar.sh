#!/usr/bin/env bash
# Prova do ALCANCE do modo servidor, com o openvpn 2.6 de verdade (painel +
# PostgreSQL + openvpn em netns). Cada caso nos dois sentidos:
#
#   failover  -- o endereco principal da rede morto (pacotes descartados, sem
#                resposta): com o alternativo no perfil conecta, e em quanto
#                tempo; sem ele (RED) nao conecta
#   queda     -- UDP do membro bloqueado: rede UDP com queda TCP 443 conecta
#                pelo bloco TCP (ponte do supervisor), com o MESMO IP fixo e
#                falando com o membro que esta no UDP; sem a queda (RED), nao
#   port-share-- a porta da queda (ponte) e a de uma rede TCP (OpenVPN)
#                divididas com um HTTPS: o curl recebe a pagina E o membro
#                conecta; sem o port-share (RED), o curl nao recebe
#   proxy     -- so o proxy alcanca o servidor, e ele pede senha (407):
#                credencial em arquivo 0600 (pela linha de comando) conecta;
#                credencial perguntada (pela gerencia, como o OpenVPN GUI)
#                conecta; sem credencial (RED), nao
#   socks     -- o mesmo por um SOCKS5 com usuario e senha
#
# Topologia: servidor (pxalc-s: PostgreSQL, painel, openvpn; 192.168.92.1 e
# o endereco morto .2), admin (pxalc-a, .11), ana (pxalc-b, .12) e o proxy
# (pxalc-p, .14). Nenhuma porta do hospedeiro; processos morrem por PID.
#
# Uso: sudo ./rodar.sh [corridas=3]   (binario: target/debug/phxvpn ou PHXVPN_BIN)
set -uo pipefail

AQUI=$(cd "$(dirname "$0")" && pwd)
RAIZ=$(cd "$AQUI/../.." && pwd)
BIN=${PHXVPN_BIN:-$RAIZ/target/debug/phxvpn}
PGBIN=$(ls -d /usr/lib/postgresql/*/bin | sort -V | tail -1)
PGPORTA=55498
PAINEL=192.168.92.1:8484
N=${1:-3}
SENHA_PROXY=senha-do-proxy-prova-5519
NS_S=pxalc-s; NS_A=pxalc-a; NS_B=pxalc-b; NS_P=pxalc-p
T=$(mktemp -d /tmp/phxvpn-prova-alcance.XXXX); chmod 755 "$T"
R=$T/r.txt; : > "$R"
anota() { echo "$1 $2" >> "$R"; echo "== $1 = $2" >&2; }

limpar() {
  set +e
  for n in $NS_A $NS_B $NS_P $NS_S; do ip netns pids $n 2>/dev/null | xargs -r kill; done
  sleep 1
  [ -d "$T/pg" ] && ip netns exec $NS_S su postgres -c "$PGBIN/pg_ctl -D $T/pg -m fast stop" >/dev/null 2>&1
  for n in $NS_A $NS_B $NS_P $NS_S; do ip netns pids $n 2>/dev/null | xargs -r kill -9; ip netns del $n 2>/dev/null; rm -rf /etc/netns/$n; done
}
trap limpar EXIT
S() { ip netns exec $NS_S "$@"; }
A() { ip netns exec $NS_A "$@"; }
B() { ip netns exec $NS_B "$@"; }
P() { ip netns exec $NS_P "$@"; }

limpar
ip netns add $NS_S; ip netns add $NS_A; ip netns add $NS_B; ip netns add $NS_P
S ip link set lo up
S ip link add pxabr type bridge; S ip link set pxabr up
S ip addr add 192.168.92.1/24 dev pxabr; S ip addr add 192.168.92.2/24 dev pxabr
i=1
for par in "$NS_A 11" "$NS_B 12" "$NS_P 14"; do
  set -- $par
  ip -n $1 link set lo up
  ip link add pxav$i netns $NS_S type veth peer name eth0 netns $1
  S ip link set pxav$i master pxabr up
  ip -n $1 addr add 192.168.92.$2/24 dev eth0; ip -n $1 link set eth0 up
  mkdir -p /etc/netns/$1; echo "192.168.92.1 vpn.prova.local" > /etc/netns/$1/hosts
  i=$((i + 1))
done
# O endereco morto: existe, mas nada responde (nem ICMP) -- servidor fora do ar.
S iptables -A INPUT -d 192.168.92.2 -j DROP
nome_aponta() { echo "$2 vpn.prova.local" > /etc/netns/$1/hosts; }

# O HTTPS que ja usava a porta.
openssl req -x509 -newkey ec -pkeyopt ec_paramgen_curve:P-256 -nodes -days 2 -subj /CN=prova \
  -keyout "$T/web.key" -out "$T/web.crt" 2>/dev/null
S python3 "$AQUI/https.py" 9443 "$T/web.crt" "$T/web.key" &

mkdir -p "$T/pg" "$T/pgsock"; chown postgres "$T/pg" "$T/pgsock"
echo "senha-pg-prova" > "$T/pgsenha"; chown postgres "$T/pgsenha"
su postgres -c "$PGBIN/initdb -D $T/pg -A scram-sha-256 --pwfile=$T/pgsenha -U postgres" >/dev/null
S su postgres -c "$PGBIN/pg_ctl -D $T/pg -o '-p $PGPORTA -k $T/pgsock -c listen_addresses=127.0.0.1' -l $T/pg/pg.log start" >/dev/null
sleep 2
ip netns exec $NS_S env PHXVPN_PG="host=127.0.0.1 port=$PGPORTA user=postgres password=senha-pg-prova dbname=postgres" \
  "$BIN" painel --dados "$T/dados" --openvpn --escutar $PAINEL --aceito-sem-tls >"$T/painel.log" 2>&1 &
for _ in $(seq 50); do grep -q "CODIGO DE INSTALACAO" "$T/painel.log" && break; sleep 0.2; done
codigo=$(grep -o "CODIGO DE INSTALACAO: [^ ]*" "$T/painel.log" | awk '{print $4}')

api() {
  S python3 - "$PAINEL" "$@" <<'PY'
import sys, urllib.request
base, m, c, tk, corpo = sys.argv[1:6]
r = urllib.request.Request("http://" + base + c, method=m,
    data=corpo.encode() if m == "POST" else None,
    headers={"Content-Type": "application/json", **({"Authorization": "Bearer " + tk} if tk else {})})
try:
    print(urllib.request.urlopen(r).read().decode())
except urllib.error.HTTPError as e:
    print(e.read().decode()); sys.exit(1)
PY
}
campo() { python3 -c "import json,sys; print(json.load(sys.stdin)['$1'])"; }
api POST /api/instalar "" "{\"codigo_instalacao\":\"$codigo\",\"empresa\":\"Prova Ltda\",\"finalidade\":\"prova\",
 \"responsavel\":\"Prova\",\"email\":\"p@prova.local\",\"telefone\":\"0\",\"admin_usuario\":\"admin\",
 \"admin_senha\":\"senha-admin-longa\",\"senha_mestre\":\"senha-mestre-longa-da-prova\",
 \"servidor_nome\":\"vpn.prova.local\",\"servidor_ip\":\"192.168.92.1\",\"servidor_dns\":\"vpn.prova.local\",\"certificado_pem\":\"\"}" >/dev/null
TKA=$(api POST /api/login "" '{"usuario":"admin","senha":"senha-admin-longa"}' | campo token)
api POST /api/redes "$TKA" '{"nome":"Matriz","senha":"senha-da-rede","finalidade":"prova"}' | campo perfil > "$T/admin.ovpn"
api POST /api/usuarios "$TKA" '{"login":"ana","senha":"senha-da-ana-longa","email":"ana@prova.local"}' >/dev/null
TKB=$(api POST /api/login "" '{"usuario":"ana","senha":"senha-da-ana-longa"}' | campo token)
REDE=$(api GET /api/redes "$TKA" "" | python3 -c "import json,sys; print(json.load(sys.stdin)[0]['id'])")
DIR=$T/dados/redes/$REDE

alcance() { # REDE_ID CAMPOS_JSON -- a resposta vai ao registro: gravar que falha nao passa calado
  echo "alcance $1 $2 -> $(api POST /api/redes/alcance/gravar "$TKA" "{\"rede_id\":$1$2}")" | tee -a "$T/alcance.log" >&2
}
perfil_ana() { # REDE CAMPOS_JSON ARQUIVO
  api POST /api/redes/entrar "$TKB" "{\"nome\":\"$1\",\"senha\":\"senha-da-rede\"$2}" | campo perfil > "$3"
}
sobe() { # NETNS PERFIL LOG [args...]
  local ns=$1 perfil=$2 log=$3; shift 3
  rm -f "$log"
  ip netns exec "$ns" openvpn --config "$perfil" --log "$log" --daemon --writepid "$log.pid" "$@"
}
para() { # LOG
  local p; p=$(cat "$1.pid" 2>/dev/null) || return 0
  kill "$p" 2>/dev/null
  for _ in $(seq 50); do kill -0 "$p" 2>/dev/null || return 0; sleep 0.1; done
  kill -9 "$p" 2>/dev/null; true
}
conectou() { grep -q "Initialization Sequence Completed" "$1" 2>/dev/null; }
esperar() { # SEGUNDOS COMANDO... -> segundos ate dar certo, ou null
  local t0 fim; t0=$(date +%s.%N); fim=$(( $(date +%s) + $1 )); shift
  while [ "$(date +%s)" -lt "$fim" ]; do
    if "$@"; then python3 -c "import sys,time; print(round(time.time()-float(sys.argv[1]),1))" "$t0"; return; fi
    sleep 0.2
  done
  echo null
}
lista() { printf '%s' "$*" | tr ' ' ','; }

# Admin no UDP o tempo todo: o outro lado do ping de quem cai para o TCP.
sobe $NS_A "$T/admin.ovpn" "$T/a.log"
anota admin_udp_conectou_s "$(esperar 30 conectou "$T/a.log")"
religa_admin() { # o OpenVPN reinicia a cada alcance gravado; o admin volta
  esperar 40 bash -c "grep -c 'Initialization Sequence Completed' '$T/a.log' | grep -qv '^$1\$'" >/dev/null
}

# ---------------------------------------------------------------- failover
nome_aponta $NS_B 192.168.92.2
alcance $REDE ',"remotos":["192.168.92.1"]' >/dev/null
perfil_ana Matriz "" "$T/f.ovpn"
anota failover_perfil_tem_alternativo "$(grep -c '^remote 192.168.92.1 1195$' "$T/f.ovpn")"
tempos=()
for k in $(seq "$N"); do
  sobe $NS_B "$T/f.ovpn" "$T/f$k.log"
  tempos+=("$(esperar 60 conectou "$T/f$k.log")")
  para "$T/f$k.log"
done
anota failover_s "[$(lista "${tempos[@]}")]"
anota failover_tentou_o_morto "$(grep -c 'link remote: \[AF_INET\]192.168.92.2:1195' "$T/f1.log")"
alcance $REDE ',"remotos":[]' >/dev/null
perfil_ana Matriz "" "$T/f0.ovpn"
sobe $NS_B "$T/f0.ovpn" "$T/f0.log"
anota red_failover_conectou_em_45s "$(esperar 45 conectou "$T/f0.log")"
para "$T/f0.log"

# ------------------------------------------------------------------- queda
nome_aponta $NS_B 192.168.92.1
S iptables -A INPUT -s 192.168.92.12 -p udp -j DROP
ca=$(grep -c "Initialization Sequence Completed" "$T/a.log")
alcance $REDE ',"queda_tcp":443,"port_share":"127.0.0.1:9443"' >/dev/null
religa_admin "$ca"
perfil_ana Matriz "" "$T/q.ovpn"
anota queda_perfil_blocos "$(grep -c '^<connection>$' "$T/q.ovpn")"
tempos=()
for k in $(seq "$N"); do
  sobe $NS_B "$T/q.ovpn" "$T/q$k.log"
  tempos+=("$(esperar 60 conectou "$T/q$k.log")")
  if [ "$k" = 1 ]; then
    anota queda_proto_da_conexao "$(grep -o 'TCPv4_CLIENT link remote' "$T/q1.log" | head -1 | tr ' ' _)"
    anota queda_ip_da_ana "$(B ip -4 -o addr show tun0 | awk '{print $4}')"
    anota queda_ping_ana_para_admin_no_udp "$(B ping -c 5 -W 2 -q 10.77.1.2 | grep -o '[0-9]* received' | awk '{print $1}')/5"
    esperar 15 grep -q '^CLIENT_LIST,ana' "$DIR/status.log" >/dev/null
    anota queda_ana_vista_pelo_openvpn_como "$(grep '^CLIENT_LIST,ana' "$DIR/status.log" | cut -d, -f3)"
    anota queda_log_liga_o_ip_de_fora "$(grep -c 'queda-tcp: 192.168.92.12:[0-9]* entra como 127\.' "$DIR/openvpn.log")"
    anota port_share_curl_pela_ponte "$(A curl -sk --max-time 5 https://192.168.92.1:443/ | tr -d '\n')"
    anota port_share_ana_segue_conectada "$(B ping -c 2 -W 2 -q 10.77.1.2 | grep -o '[0-9]* received' | awk '{print $1}')/2"
  fi
  para "$T/q$k.log"
done
anota queda_s "[$(lista "${tempos[@]}")]"
ca=$(grep -c "Initialization Sequence Completed" "$T/a.log")
alcance $REDE ',"queda_tcp":443' >/dev/null
religa_admin "$ca"
anota red_port_share_curl_sem_port_share "\"$(A curl -sk --max-time 5 https://192.168.92.1:443/ | tr -d '\n')\""
ca=$(grep -c "Initialization Sequence Completed" "$T/a.log")
alcance $REDE '' >/dev/null
religa_admin "$ca"
perfil_ana Matriz "" "$T/q0.ovpn"
sobe $NS_B "$T/q0.ovpn" "$T/q0.log"
anota red_queda_conectou_em_45s "$(esperar 45 conectou "$T/q0.log")"
para "$T/q0.log"
anota red_queda_porta_443_escuta "$(S ss -Htln 'sport = :443' | wc -l)"

# --------------------------------------------------- port-share, rede TCP
api POST /api/redes "$TKA" '{"nome":"Hotel","senha":"senha-da-rede","protocolo":"tcp","porta":8443}' >/dev/null
HOTEL=$(api GET /api/redes "$TKA" "" | python3 -c "import json,sys; print([r['id'] for r in json.load(sys.stdin) if r['nome']=='Hotel'][0])")
alcance $HOTEL ',"port_share":"127.0.0.1:9443"' >/dev/null
anota port_share_conf_tcp "$(grep -c '^port-share 127.0.0.1 9443$' "$T/dados/redes/$HOTEL/servidor.conf")"
perfil_ana Hotel "" "$T/h.ovpn"
sobe $NS_B "$T/h.ovpn" "$T/h.log"
anota port_share_tcp_ana_conectou_s "$(esperar 45 conectou "$T/h.log")"
anota port_share_curl_pelo_openvpn "$(A curl -sk --max-time 5 https://192.168.92.1:8443/ | tr -d '\n')"
para "$T/h.log"
alcance $HOTEL '' >/dev/null
sleep 2
anota red_port_share_tcp_curl "\"$(A curl -sk --max-time 5 https://192.168.92.1:8443/ | tr -d '\n')\""

# ------------------------------------------------------------ proxy HTTP
# Dali em diante a ana so alcanca o servidor pelo proxy (e o painel, 8484).
S iptables -A INPUT -s 192.168.92.12 -p tcp --dport 443 -j DROP
ca=$(grep -c "Initialization Sequence Completed" "$T/a.log")
alcance $REDE ',"queda_tcp":443' >/dev/null
religa_admin "$ca"
P python3 "$RAIZ/provas/tcp/proxy.py" 3128 "prova:$SENHA_PROXY" "$T/proxy.log" &
P python3 "$AQUI/socks.py" 1080 "prova:$SENHA_PROXY" "$T/socks.log" &
sleep 1
cli_entrar() { # SAIDA ARGS...
  local saida=$1; shift
  B env PHXVPN_ACEITO_SEM_TLS=1 PHXVPN_SENHA=senha-da-ana-longa PHXVPN_SENHA_REDE=senha-da-rede PHXVPN_SENHA_PROXY=$SENHA_PROXY \
    "$BIN" entrar --painel "http://$PAINEL" --usuario ana --rede Matriz --saida "$saida" "$@" >/dev/null
}
cli_entrar "$T/px.ovpn" --http-proxy 192.168.92.14:3128 --proxy-usuario prova
anota proxy_arquivo_modo "$(stat -c %a "$T/px.ovpn.proxy")"
anota proxy_senha_no_perfil "$(grep -c "$SENHA_PROXY" "$T/px.ovpn")"
anota proxy_senha_no_painel "$(grep -rl "$SENHA_PROXY" "$T/dados" "$T/painel.log" 2>/dev/null | wc -l)"
anota proxy_linha "\"$(grep '^http-proxy' "$T/px.ovpn")\""
sobe $NS_B "$T/px.ovpn" "$T/px.log"
anota proxy_arquivo_conectou_s "$(esperar 60 conectou "$T/px.log")"
para "$T/px.log"
perfil_ana Matriz ',"proxy":"192.168.92.14:3128","proxy_credencial":"perguntar"' "$T/pxp.ovpn"
anota proxy_perguntar_linha "\"$(grep '^http-proxy' "$T/pxp.ovpn")\""
echo "$SENHA_PROXY" > "$T/senha-gui"; chmod 600 "$T/senha-gui"
sobe $NS_B "$T/pxp.ovpn" "$T/pxp.log" --management 127.0.0.1 7505 --management-query-passwords
B python3 "$AQUI/gerencia.py" 7505 prova "$T/senha-gui" "$T/gerencia.log" &
anota proxy_perguntar_conectou_s "$(esperar 60 conectou "$T/pxp.log")"
anota proxy_perguntar_pedidos "$(grep -c "Need 'HTTP Proxy'" "$T/gerencia.log")"
para "$T/pxp.log"
perfil_ana Matriz ',"proxy":"192.168.92.14:3128"' "$T/px0.ovpn"
sobe $NS_B "$T/px0.ovpn" "$T/px0.log"
anota red_proxy_sem_credencial_conectou_em_45s "$(esperar 45 conectou "$T/px0.log")"
para "$T/px0.log"
anota proxy_registro "\"$(sort "$T/proxy.log" | uniq -c | tr -s ' ' | tr '\n' ';')\""

# ------------------------------------------------------------------- SOCKS
cli_entrar "$T/sk.ovpn" --socks-proxy 192.168.92.14:1080 --proxy-usuario prova
anota socks_linha "\"$(grep '^socks-proxy' "$T/sk.ovpn")\""
sobe $NS_B "$T/sk.ovpn" "$T/sk.log"
anota socks_arquivo_conectou_s "$(esperar 60 conectou "$T/sk.log")"
anota socks_ping_ana_para_admin "$(B ping -c 3 -W 2 -q 10.77.1.2 | grep -o '[0-9]* received' | awk '{print $1}')/3"
para "$T/sk.log"
perfil_ana Matriz ',"proxy":"192.168.92.14:1080","proxy_tipo":"socks"' "$T/sk0.ovpn"
sobe $NS_B "$T/sk0.ovpn" "$T/sk0.log"
anota red_socks_sem_credencial_conectou_em_45s "$(esperar 45 conectou "$T/sk0.log")"
para "$T/sk0.log"
anota socks_registro "\"$(sort "$T/socks.log" | uniq -c | tr -s ' ' | tr '\n' ';')\""

cp "$T/alcance.log" "$AQUI/alcance.log"
anota painel_erros "$(grep -ci 'erro\|panic' "$T/painel.log")"
cp "$R" "$AQUI/medidas.txt"

python3 - "$R" "$AQUI/resultados.json" "$N" "$(openvpn --version | head -1 | awk '{print $2}')" <<'PY'
import json, sys, datetime, platform
m = {}
for l in open(sys.argv[1]):
    k, v = l.rstrip("\n").split(" ", 1)
    try:
        m[k] = json.loads(v)
    except ValueError:
        m[k] = v
num = lambda v: isinstance(v, (int, float)) and not isinstance(v, bool)
tudo = lambda l: isinstance(l, list) and l and all(num(x) for x in l)
verde = {
    "failover": tudo(m["failover_s"]) and m["failover_tentou_o_morto"] >= 1,
    "queda": tudo(m["queda_s"]) and "TCP" in str(m["queda_proto_da_conexao"])
             and m["queda_ip_da_ana"] == "10.77.1.3/24" and m["queda_ping_ana_para_admin_no_udp"] == "5/5"
             and str(m["queda_ana_vista_pelo_openvpn_como"]).startswith("127.")
             and not str(m["queda_ana_vista_pelo_openvpn_como"]).startswith("127.0.0.1:")
             and m["queda_log_liga_o_ip_de_fora"] >= 1,
    "port_share": m["port_share_curl_pela_ponte"] == "phxvpn-prova-https"
                  and m["port_share_ana_segue_conectada"] == "2/2"
                  and m["port_share_curl_pelo_openvpn"] == "phxvpn-prova-https"
                  and num(m["port_share_tcp_ana_conectou_s"]),
    "proxy": num(m["proxy_arquivo_conectou_s"]) and num(m["proxy_perguntar_conectou_s"])
             and m["proxy_arquivo_modo"] == 600 and m["proxy_senha_no_perfil"] == 0
             and m["proxy_senha_no_painel"] == 0 and m["proxy_perguntar_pedidos"] >= 1,
    "socks": num(m["socks_arquivo_conectou_s"]) and m["socks_ping_ana_para_admin"] == "3/3",
}
red = {
    "failover_sem_alternativo": m["red_failover_conectou_em_45s"] is None,
    "queda_sem_queda": m["red_queda_conectou_em_45s"] is None and m["red_queda_porta_443_escuta"] == 0,
    "port_share_ponte_sem_port_share": m["red_port_share_curl_sem_port_share"] == "",
    "port_share_tcp_sem_port_share": m["red_port_share_tcp_curl"] == "",
    "proxy_sem_credencial": m["red_proxy_sem_credencial_conectou_em_45s"] is None,
    "socks_sem_credencial": m["red_socks_sem_credencial_conectou_em_45s"] is None,
}
r = {
    "prova": "alcance do modo servidor: failover de remote, queda UDP->TCP pela ponte, port-share, proxy HTTP e SOCKS com credencial",
    "medido_em": datetime.datetime.now(datetime.timezone.utc).isoformat(timespec="seconds"),
    "kernel": platform.release(),
    "openvpn": sys.argv[4],
    "corridas_failover_e_queda": int(sys.argv[3]),
    "medidas": m,
    "confere_verde": verde,
    "confere_red": red,
}
open(sys.argv[2], "w").write(json.dumps(r, ensure_ascii=False, indent=1) + "\n")
print(json.dumps({"verde": verde, "red": red}, ensure_ascii=False))
sys.exit(0 if all(verde.values()) and all(red.values()) else 1)
PY
