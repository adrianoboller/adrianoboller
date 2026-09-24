#!/usr/bin/env bash
# Prova do TUNEL TOTAL e do DNS da rede (saida.rs, dns.rs) com o openvpn
# 2.6.19 DE VERDADE, painel + PostgreSQL + supervisor, em netns proprios:
#
#   S  servidor (painel, PostgreSQL, os openvpn das redes); wan 192.0.2.1,
#      LAN da empresa 192.168.10.1 (H)
#   I  «internet»: roteia a LAN de casa da ana (192.168.50.0/24) e a do
#      xavier ate o servidor; tem o SITE externo 198.51.100.10:9000 (devolve
#      o IP de quem conectou) e um DNS externo 198.51.100.53 (anota quem
#      perguntou). 192.168.50.5 e a «impressora» da LAN de casa da ana.
#   H  host da LAN atras do servidor: 192.168.10.5, sem rota padrao
#   A  membro «ana» da rede Matriz (casa: 192.168.50.11, gw 192.168.50.1)
#   X  membro «xavier» da rede Outra (casa: 192.168.51.11)
#
# Casos, cada um nos dois sentidos:
#   sem tunel total o site ve a ana pelo IP de casa; com, pelo IP do servidor
#     (e o SYN aparece no tcpdump da wan do servidor, nao no da casa); RED:
#     sem o NAT de saida, a ana nao chega ao site
#   tunel total nao da LAN: ana -> H (LAN do servidor, sem rota) 0/3; RED com
#     o tunel total «ingenuo» (accept + masquerade sem excecao): 3/3
#   isolamento: ana (tunel total) -> xavier (outra rede) 0/3
#   block-local: sem ele a impressora de casa responde; com, nao
#   DNS: sem os nomes, ana.matriz.phx nao resolve; com, resolve (e o nome
#     curto, pela search), o nome externo sai pelo DNS do servidor; o xavier
#     perguntando ao 10.77.1.1 fica sem resposta
#   IPv6 (kernel sem IPv6, ipv6.disable=1): perfil sem a marca «sem IPv6»
#     MORRE no ifconfig-ipv6 (FATAL); com a marca, conecta
#   block-outside-dns: empurrado, o Linux recusa a opcao e segue
#   fim: tudo desligado, regras de volta ao numero de antes e ip_forward 0
#
# Uso (root):  MOTOR=nft ./rodar.sh (padrao) | MOTOR=iptables ./rodar.sh
set -euo pipefail

AQUI=$(cd "$(dirname "$0")" && pwd)
RAIZ=$(cd "$AQUI/../.." && pwd)
MOTOR=${MOTOR:-nft}
T=${1:-$(mktemp -d /tmp/phxvpn-prova-tt.XXXX)}
BIN=${PHXVPN_BIN:-$RAIZ/target/debug/phxvpn}
PGBIN=$(ls -d /usr/lib/postgresql/*/bin | sort -V | tail -1)
PGPORTA=55494
PAINEL=127.0.0.1:8482
P=pxtt
NSS="$P-s $P-i $P-h $P-a $P-x"
mkdir -p "$T"; chmod 755 "$T"
echo "== pasta: $T (motor $MOTOR)"

limpar() {
  set +e
  for n in $NSS; do [ "$n" = $P-s ] || ip netns pids $n 2>/dev/null | xargs -r kill; done
  ip netns pids $P-s 2>/dev/null | xargs -r kill
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
par s wan 192.0.2.1/24 i s0 192.0.2.254/24
par s lan 192.168.10.1/24 h eth0 192.168.10.5/24
par a eth0 192.168.50.11/24 i a0 192.168.50.1/24
par x eth0 192.168.51.11/24 i x0 192.168.51.1/24
ns i ip addr add 192.168.50.5/24 dev a0
ns i ip addr add 198.51.100.10/32 dev lo
ns i ip addr add 198.51.100.53/32 dev lo
ns i sh -c 'echo 1 > /proc/sys/net/ipv4/ip_forward'
ns s ip route add default via 192.0.2.254
ns a ip route add default via 192.168.50.1
ns x ip route add default via 192.168.51.1
for n in s a x; do mkdir -p /etc/netns/$P-$n; echo "nameserver 198.51.100.53" > /etc/netns/$P-$n/resolv.conf; done

# --- a «internet»: site e DNS externo
ns i python3 -c "
import socket
s=socket.socket(); s.setsockopt(socket.SOL_SOCKET,socket.SO_REUSEADDR,1); s.bind(('198.51.100.10',9000)); s.listen()
while True:
    c,a=s.accept(); c.sendall(a[0].encode()); c.close()" &
ns i python3 - "$T/dns-externo.log" <<'PY' &
import socket, sys, struct
log = open(sys.argv[1], "a", buffering=1)
s = socket.socket(socket.AF_INET, socket.SOCK_DGRAM); s.bind(("198.51.100.53", 53))
while True:
    q, de = s.recvfrom(512)
    i, rot = 12, []
    while q[i]:
        rot.append(q[i+1:i+1+q[i]].decode()); i += 1 + q[i]
    nome, fim = ".".join(rot).lower(), i + 5
    log.write(f"{de[0]} {nome}\n")
    tipo = struct.unpack(">H", q[i+1:i+3])[0]
    if nome == "site.externo.teste" and tipo == 1:
        r = q[:2] + b"\x81\x80\x00\x01\x00\x01\x00\x00\x00\x00" + q[12:fim] + b"\xc0\x0c\x00\x01\x00\x01\x00\x00\x00\x3c\x00\x04" + bytes([198,51,100,10])
    else:
        r = q[:2] + b"\x81\x83\x00\x01\x00\x00\x00\x00\x00\x00" + q[12:fim]
    s.sendto(r, de)
PY
ns h python3 -c "
import socket
s=socket.socket(); s.setsockopt(socket.SOL_SOCKET,socket.SO_REUSEADDR,1); s.bind(('0.0.0.0',9000)); s.listen()
while True:
    c,a=s.accept(); c.sendall(a[0].encode()); c.close()" &

# --- PostgreSQL descartavel no netns do servidor
mkdir -p "$T/pg"; chown postgres "$T/pg"
echo "senha-pg-prova" > "$T/pgsenha"; chown postgres "$T/pgsenha"
su postgres -c "$PGBIN/initdb -D $T/pg -A scram-sha-256 --pwfile=$T/pgsenha -U postgres" >/dev/null
ns s su postgres -c "$PGBIN/pg_ctl -D $T/pg -o \"-p $PGPORTA -c unix_socket_directories='' -c listen_addresses=127.0.0.1\" -l $T/pg/pg.log start" >/dev/null
sleep 2

PHXVPN_FIREWALL=$MOTOR PHXVPN_PG="host=127.0.0.1 port=$PGPORTA user=postgres password=senha-pg-prova dbname=postgres" \
  ns s "$BIN" painel --dados "$T/dados" --openvpn --escutar $PAINEL >"$T/painel.log" 2>&1 &
for _ in $(seq 50); do grep -q "CODIGO DE INSTALACAO" "$T/painel.log" && break; sleep 0.2; done
CODIGO=$(grep -o "CODIGO DE INSTALACAO: [^ ]*" "$T/painel.log" | awk '{print $4}')

api() { # api METODO CAMINHO TOKEN JSON -> corpo (erro: {"erro":...}, status 1)
  ns s python3 - "$@" <<'PY'
import json, sys, urllib.request
m, c, tk, corpo = sys.argv[1:5]
r = urllib.request.Request("http://127.0.0.1:8482" + c, method=m,
    data=corpo.encode() if m == "POST" else None,
    headers={"Content-Type": "application/json", **({"Authorization": "Bearer " + tk} if tk else {})})
try:
    print(urllib.request.urlopen(r, timeout=60).read().decode())
except urllib.error.HTTPError as e:
    print(e.read().decode()); sys.exit(1)
PY
}
campo() { python3 -c "import json,sys; print(json.load(sys.stdin)$1)"; }

api POST /api/instalar "" "{\"codigo_instalacao\":\"$CODIGO\",\"empresa\":\"Prova Ltda\",\"finalidade\":\"prova\",
 \"responsavel\":\"Prova\",\"email\":\"p@prova.local\",\"telefone\":\"0\",\"admin_usuario\":\"admin\",
 \"admin_senha\":\"senha-admin-longa\",\"senha_mestre\":\"senha-mestre-longa-da-prova\",
 \"servidor_nome\":\"vpn.prova.local\",\"servidor_ip\":\"192.0.2.1\",\"servidor_dns\":\"\",\"certificado_pem\":\"\"}" >/dev/null
TK=$(api POST /api/login "" '{"usuario":"admin","senha":"senha-admin-longa"}' | campo "['token']")
api POST /api/redes "$TK" '{"nome":"Matriz","senha":"senha-da-matriz","finalidade":"prova"}' >/dev/null
api POST /api/redes "$TK" '{"nome":"Outra","senha":"senha-da-outra","finalidade":"isolamento"}' >/dev/null
membro() { # LOGIN REDE SENHA_REDE ARQUIVO EXTRA_JSON -> grava o perfil
  api POST /api/usuarios "$TK" "{\"login\":\"$1\",\"senha\":\"senha-de-$1-longa\"}" >/dev/null 2>&1 || true
  local tk; tk=$(api POST /api/login "" "{\"usuario\":\"$1\",\"senha\":\"senha-de-$1-longa\"}" | campo "['token']")
  api POST /api/redes/entrar "$tk" "{\"nome\":\"$2\",\"senha\":\"$3\"$5}" | campo "['perfil']" > "$T/$4"
}
# O perfil da ana pede o script de DNS e diz que a maquina nao tem IPv6.
membro ana Matriz senha-da-matriz ana.ovpn ',"dns_linux":"resolvconf","sem_ipv6":true'
PERFIL_TEM_SCRIPT=$(grep -c -e '^script-security 2$' -e '^up /etc/openvpn/update-resolv-conf$' -e '^pull-filter ignore "ifconfig-ipv6"$' "$T/ana.ovpn" || true)
# Na prova, o update-resolv-conf (que pede o `resolvconf`) vira o substituto.
sed -i "s#^up /etc/openvpn/update-resolv-conf\$#up \"$AQUI/up-dns.sh $T\"#; s#^down /etc/openvpn/update-resolv-conf\$#down \"$AQUI/up-dns.sh $T\"#" "$T/ana.ovpn"
chmod 755 "$AQUI/up-dns.sh"
membro xavier Outra senha-da-outra xavier.ovpn ''
RECUSA_DNS_LINUX=$(api POST /api/redes/entrar "$(api POST /api/login "" '{"usuario":"ana","senha":"senha-de-ana-longa"}' | campo "['token']")" \
  '{"nome":"Matriz","senha":"senha-da-matriz","dns_linux":"resolvconf\nup /bin/sh"}' || true)
MATRIZ=$(api GET /api/redes "$TK" "" | python3 -c "import json,sys; print([r['id'] for r in json.load(sys.stdin) if r['nome']=='Matriz'][0])")
sleep 1

conectar() { # NS LOGIN [ARQ] -> espera o tunel; 1 se o cliente morreu
  local log="$T/c-$2.log" arq=${3:-$2.ovpn}
  [ -f "$T/c-$2.pid" ] && { kill "$(cat "$T/c-$2.pid")" 2>/dev/null || true; sleep 1.5; }
  : > "$log"
  ns "$1" openvpn --config "$T/$arq" --daemon --log "$log" --writepid "$T/c-$2.pid"
  for _ in $(seq 100); do
    grep -q "Initialization Sequence Completed" "$log" 2>/dev/null && return 0
    grep -q "Exiting due to fatal error" "$log" 2>/dev/null && return 1
    sleep 0.2
  done
  echo "FALHOU: $2 nao conectou"; tail -5 "$log"; exit 1
}
ip_tun() { ns "$1" ip -4 -o addr show dev tun0 | awk '{print $4}' | cut -d/ -f1; }
pings() { # NS DESTINO -> recebidos de 3
  local n; n=$(ns "$1" ping -c 3 -W 1 -i 0.3 "$2" 2>/dev/null | awk '/received/ {print $4}' || true)
  echo "${n:-0}"
}
tcp() { # NS DESTINO -> o IP de quem conectou, visto pelo destino; ou "falhou"
  ns "$1" python3 -c "
import socket
try:
    s=socket.create_connection(('$2',9000),timeout=3); print(s.recv(64).decode() or 'vazio')
except Exception as e: print('falhou')"
}
site() { # -> "IP_visto_pelo_site syn_na_wan_do_servidor syn_na_casa_da_ana"
  rm -f "$T/td-s0" "$T/td-a0"
  ns i timeout 6 tcpdump -l -n -i s0 'tcp[tcpflags] & tcp-syn != 0 and dst host 198.51.100.10 and dst port 9000' >"$T/td-s0" 2>/dev/null &
  local p1=$!
  ns i timeout 6 tcpdump -l -n -i a0 'tcp[tcpflags] & tcp-syn != 0 and dst host 198.51.100.10 and dst port 9000' >"$T/td-a0" 2>/dev/null &
  local p2=$!
  sleep 1.5
  local visto; visto=$(tcp a 198.51.100.10)
  wait $p1 $p2 || true
  echo "$visto $(grep -c ' > 198.51.100.10.9000' "$T/td-s0" || true) $(grep -c ' > 198.51.100.10.9000' "$T/td-a0" || true)"
}
resolve() { # NS NOME -> IP ou "nao"
  local r; r=$(ns "$1" getent hosts "$2" 2>/dev/null | awk '{print $1}' | head -1 || true); echo "${r:-nao}"
}
regras() {
  local nossas
  if [ "$MOTOR" = nft ]; then
    nossas=$(ns s nft -a list table ip phxvpn 2>/dev/null | grep '# handle' | grep -vc -e 'chain ' -e 'table ' || true)
  else
    nossas=$( { ns s iptables -S PHXVPN-ENC 2>/dev/null; ns s iptables -t nat -S PHXVPN-NAT 2>/dev/null; } | grep -c '^-A' || true)
  fi
  echo "{\"nossas\": ${nossas:-0}, \"ip_forward\": $(ns s cat /proc/sys/net/ipv4/ip_forward)}"
}
saida() { # JSON (sem rede_id) -> resposta
  api POST /api/redes/saida/definir "$TK" "{\"rede_id\":$MATRIZ,$1}" || true
}

# ------------------------------------------------------ sem tunel total ----
ANTES=$(regras); echo "== regras antes: $ANTES"
conectar a ana
conectar x xavier
read -r SEM_VISTO SEM_S0 SEM_A0 <<<"$(site)"
SEM_IMPRESSORA=$(pings a 192.168.50.5)
SEM_NOME=$(resolve a ana.matriz.phx)
SEM_RESOLV=$(ns a cat /etc/resolv.conf | tr '\n' ' ')
echo "== sem tunel total: site viu $SEM_VISTO (SYN wan do servidor $SEM_S0, casa $SEM_A0); impressora $SEM_IMPRESSORA/3; ana.matriz.phx=$SEM_NOME"

# -------------------------------------------------- recusas pela API ----
RECUSA_SEM_DNS=$(saida '"tunel_total":true')
RECUSA_LOCAL_SO=$(saida '"bloquear_local":true,"dns_nomes":true')
RECUSA_DNS_PRIVADO=$(saida '"tunel_total":true,"dns_empresa":"192.168.10.53"')
TK_ANA=$(api POST /api/login "" '{"usuario":"ana","senha":"senha-de-ana-longa"}' | campo "['token']")
RECUSA_NAO_ADMIN=$(api POST /api/redes/saida/definir "$TK_ANA" "{\"rede_id\":$MATRIZ,\"tunel_total\":true,\"dns_nomes\":true}" || true)

# ------------------------------------------------------ com tunel total ----
R_LIGA=$(saida '"tunel_total":true,"dns_nomes":true'); echo "== liga: $R_LIGA"
COM=$(regras)
CONF_SAIDA=$(sed -n '/# saida da rede/,$p' "$T/dados/redes/$MATRIZ/servidor.conf" | grep -e push -e block || true)
conectar a ana
conectar x xavier
ROTAS_A=$(ns a ip route | tr '\n' ';')
read -r COM_VISTO COM_S0 COM_A0 <<<"$(site)"
COM_IMPRESSORA=$(pings a 192.168.50.5)
COM_LAN_SERVIDOR=$(pings a 192.168.10.5)
COM_LAN_SERVIDOR_TCP=$(tcp a 192.168.10.5)
COM_OUTRA_REDE=$(pings a "$(ip_tun x)")
A_RESOLV=$(ns a cat /etc/resolv.conf | tr '\n' ' ')
: > "$T/dns-externo.log"
NOME_ANA=$(resolve a ana.matriz.phx)
NOME_CURTO=$(resolve a ana)
NOME_XAVIER_NA_MATRIZ=$(resolve a xavier.matriz.phx)
NOME_EXTERNO=$(resolve a site.externo.teste)
DNS_EXTERNO_VIU=$(sort -u "$T/dns-externo.log" | tr '\n' ';')
IP_ANA=$(ip_tun a)
# O xavier (rede Outra) pergunta direto ao resolvedor da Matriz. Com rota
# manual o pacote CHEGA ao servidor (o ping responde: e endereco local dele);
# quem cala e o resolvedor, pela origem.
ns x ip route add 10.77.$MATRIZ.0/24 dev tun0
XAVIER_PING_RESOLVEDOR=$(pings x 10.77.$MATRIZ.1)
XAVIER_PERGUNTA=$(ns x python3 -c "
import socket
q=b'\x12\x34\x01\x00\x00\x01\x00\x00\x00\x00\x00\x00\x03ana\x06matriz\x03phx\x00\x00\x01\x00\x01'
s=socket.socket(socket.AF_INET,socket.SOCK_DGRAM); s.settimeout(2)
try:
    s.sendto(q,('10.77.$MATRIZ.1',53)); s.recv(512); print('respondeu')
except Exception: print('sem resposta')")
ANA_PERGUNTA_DIRETO=$(ns a python3 -c "
import socket
q=b'\x12\x34\x01\x00\x00\x01\x00\x00\x00\x00\x00\x00\x03ana\x06matriz\x03phx\x00\x00\x01\x00\x01'
s=socket.socket(socket.AF_INET,socket.SOCK_DGRAM); s.settimeout(2)
try:
    s.sendto(q,('10.77.$MATRIZ.1',53)); r=s.recv(512); print('.'.join(map(str,r[-4:])))
except Exception: print('sem resposta')")
LOG_BLOCK_OUTSIDE=$(grep -c "Unrecognized option or missing or extra parameter(s) in \[PUSH-OPTIONS\]:[0-9]*: block-outside-dns" "$T/c-ana.log" || true)
LOG_FILTRO_IPV6=$(grep -c "Pushed option removed by filter: 'ifconfig-ipv6" "$T/c-ana.log" || true)
echo "== com tunel total: site viu $COM_VISTO (SYN wan $COM_S0, casa $COM_A0); impressora $COM_IMPRESSORA/3; LAN do servidor $COM_LAN_SERVIDOR/3 ($COM_LAN_SERVIDOR_TCP); outra rede $COM_OUTRA_REDE/3"
echo "== DNS: ana.matriz.phx=$NOME_ANA (ip da ana $IP_ANA) curto=$NOME_CURTO externo=$NOME_EXTERNO; DNS externo viu: $DNS_EXTERNO_VIU; xavier->10.77.$MATRIZ.1: $XAVIER_PERGUNTA"

# RED 1: sem o NAT de saida a ana nao chega ao site.
if [ "$MOTOR" = nft ]; then ns s nft flush chain ip phxvpn saida; else ns s iptables -t nat -F PHXVPN-NAT; fi
RED_SEM_NAT=$(tcp a 198.51.100.10)
# RED 2: tunel total «ingenuo» (aceita e mascara tudo): a LAN do servidor abre.
saida '"tunel_total":true,"dns_nomes":true' >/dev/null
if [ "$MOTOR" = nft ]; then
  ns s nft insert rule ip phxvpn encaminhar ip saddr 10.77.$MATRIZ.0/24 accept
  ns s nft insert rule ip phxvpn saida ip saddr 10.77.$MATRIZ.0/24 masquerade
else
  ns s iptables -I PHXVPN-ENC 1 -s 10.77.$MATRIZ.0/24 -j ACCEPT
  ns s iptables -t nat -I PHXVPN-NAT 1 -s 10.77.$MATRIZ.0/24 -j MASQUERADE
fi
conectar a ana
RED_LAN_SERVIDOR=$(pings a 192.168.10.5)
RED_LAN_SERVIDOR_TCP=$(tcp a 192.168.10.5)
echo "== RED: sem NAT o site: $RED_SEM_NAT; tunel ingenuo: LAN do servidor $RED_LAN_SERVIDOR/3 ($RED_LAN_SERVIDOR_TCP)"

# ------------------------------------------------------------ block-local ----
R_LOCAL=$(saida '"tunel_total":true,"bloquear_local":true,"dns_nomes":true')
conectar a ana
LOCAL_IMPRESSORA=$(pings a 192.168.50.5)
LOCAL_GATEWAY=$(pings a 192.168.50.1)
read -r LOCAL_VISTO _ _ <<<"$(site)"
echo "== block-local: impressora $LOCAL_IMPRESSORA/3, gateway de casa $LOCAL_GATEWAY/3, site viu $LOCAL_VISTO"

# ----------------------------------------------- IPv6 (kernel sem IPv6) ----
membro ana Matriz senha-da-matriz ana-sem-marca.ovpn ''
SEM_MARCA=0; conectar a ana-sem-marca ana-sem-marca.ovpn || SEM_MARCA=$?
LOG_FATAL=$(grep -c "Linux can't add IPv6 to interface" "$T/c-ana-sem-marca.log" || true)
[ -f "$T/c-ana-sem-marca.pid" ] && kill "$(cat "$T/c-ana-sem-marca.pid")" 2>/dev/null || true
echo "== sem a marca sem IPv6: conectar=$SEM_MARCA (1 = morreu), FATAL no log=$LOG_FATAL"

# ------------------------------------------------ DNS da empresa, sem nomes ----
membro ana Matriz senha-da-matriz ana.ovpn ',"dns_linux":"resolvconf","sem_ipv6":true'
sed -i "s#^up /etc/openvpn/update-resolv-conf\$#up \"$AQUI/up-dns.sh $T\"#; s#^down /etc/openvpn/update-resolv-conf\$#down \"$AQUI/up-dns.sh $T\"#" "$T/ana.ovpn"
R_EMPRESA=$(saida '"tunel_total":true,"dns_empresa":"198.51.100.53"')
conectar a ana
: > "$T/dns-externo.log"
EMPRESA_EXTERNO=$(resolve a site.externo.teste)
EMPRESA_VIU_TT=$(sort -u "$T/dns-externo.log" | awk '{print $1}' | tr '\n' ';')
EMPRESA_NOME_ANA=$(resolve a ana.matriz.phx)
R_EMPRESA_SO=$(saida '"dns_empresa":"198.51.100.53"')
conectar a ana
: > "$T/dns-externo.log"
resolve a site.externo.teste >/dev/null
EMPRESA_VIU_SEM_TT=$(sort -u "$T/dns-externo.log" | awk '{print $1}' | tr '\n' ';')
echo "== DNS da empresa: com tunel total o DNS externo viu $EMPRESA_VIU_TT; sem tunel total viu $EMPRESA_VIU_SEM_TT; ana.matriz.phx sem os nomes=$EMPRESA_NOME_ANA"

# -------------------------------------------------------------------- fim ----
R_DESLIGA=$(saida '"tunel_total":false')
DEPOIS=$(regras); echo "== regras depois de desligar: $DEPOIS"
conectar a ana
read -r FIM_VISTO _ _ <<<"$(site)"
FIM_RESOLV=$(ns a cat /etc/resolv.conf | tr '\n' ' ')
FIM_NOME=$(resolve a ana.matriz.phx)

export ANTES COM DEPOIS PERFIL_TEM_SCRIPT RECUSA_DNS_LINUX SEM_VISTO SEM_S0 SEM_A0 SEM_IMPRESSORA SEM_NOME SEM_RESOLV \
  RECUSA_SEM_DNS RECUSA_LOCAL_SO RECUSA_DNS_PRIVADO RECUSA_NAO_ADMIN R_LIGA CONF_SAIDA ROTAS_A COM_VISTO COM_S0 COM_A0 \
  COM_IMPRESSORA COM_LAN_SERVIDOR COM_LAN_SERVIDOR_TCP COM_OUTRA_REDE A_RESOLV NOME_ANA NOME_CURTO NOME_XAVIER_NA_MATRIZ \
  NOME_EXTERNO DNS_EXTERNO_VIU IP_ANA XAVIER_PING_RESOLVEDOR XAVIER_PERGUNTA ANA_PERGUNTA_DIRETO LOG_BLOCK_OUTSIDE LOG_FILTRO_IPV6 RED_SEM_NAT \
  RED_LAN_SERVIDOR RED_LAN_SERVIDOR_TCP R_LOCAL LOCAL_IMPRESSORA LOCAL_GATEWAY LOCAL_VISTO SEM_MARCA LOG_FATAL \
  R_EMPRESA EMPRESA_EXTERNO EMPRESA_VIU_TT EMPRESA_NOME_ANA R_EMPRESA_SO EMPRESA_VIU_SEM_TT R_DESLIGA FIM_VISTO FIM_RESOLV FIM_NOME
python3 - "$AQUI/resultados.json" "$MOTOR" <<'PY'
import json, sys, os, datetime
caminho, motor = sys.argv[1], sys.argv[2]
E = os.environ
j = lambda t: json.loads(t) if t.strip().startswith("{") else t
erro = lambda t: isinstance(j(t), dict) and "erro" in j(t)
r = {
  "data": datetime.datetime.now(datetime.timezone.utc).isoformat(timespec="seconds"),
  "regras": {"antes": j(E['ANTES']), "com_tunel_total": j(E['COM']), "depois_de_desligar": j(E['DEPOIS'])},
  "perfil_da_ana_linhas_pedidas": int(E['PERFIL_TEM_SCRIPT'] or 0),
  "conf_do_servidor": E['CONF_SAIDA'].splitlines(),
  "sem_tunel_total": {"site_viu": E['SEM_VISTO'], "syn_na_wan_do_servidor": int(E['SEM_S0']), "syn_na_casa": int(E['SEM_A0']),
    "impressora_de_casa": E['SEM_IMPRESSORA'] + "/3", "ana.matriz.phx": E['SEM_NOME'], "resolv_conf": E['SEM_RESOLV']},
  "com_tunel_total": {"resposta": j(E['R_LIGA']), "rotas_da_ana": E['ROTAS_A'].split(";"),
    "site_viu": E['COM_VISTO'], "syn_na_wan_do_servidor": int(E['COM_S0']), "syn_na_casa": int(E['COM_A0']),
    "impressora_de_casa_sem_block_local": E['COM_IMPRESSORA'] + "/3",
    "lan_do_servidor_sem_rota_ping": E['COM_LAN_SERVIDOR'] + "/3", "lan_do_servidor_sem_rota_tcp": E['COM_LAN_SERVIDOR_TCP'],
    "membro_de_outra_rede": E['COM_OUTRA_REDE'] + "/3",
    "log_block_outside_dns_recusado_e_seguiu": int(E['LOG_BLOCK_OUTSIDE'] or 0),
    "log_ifconfig_ipv6_filtrado": int(E['LOG_FILTRO_IPV6'] or 0)},
  "dns_nomes": {"resolv_conf_da_ana": E['A_RESOLV'], "ip_da_ana": E['IP_ANA'], "ana.matriz.phx": E['NOME_ANA'],
    "ana_nome_curto_pela_search": E['NOME_CURTO'], "xavier.matriz.phx_de_outra_rede": E['NOME_XAVIER_NA_MATRIZ'],
    "site.externo.teste": E['NOME_EXTERNO'], "dns_externo_viu": [x for x in E['DNS_EXTERNO_VIU'].split(";") if x],
    "xavier_ping_ao_resolvedor_da_matriz": E['XAVIER_PING_RESOLVEDOR'] + "/3",
    "xavier_pergunta_ao_resolvedor_da_matriz": E['XAVIER_PERGUNTA'], "ana_pergunta_direto": E['ANA_PERGUNTA_DIRETO']},
  "RED": {"sem_nat_de_saida_site": E['RED_SEM_NAT'],
    "tunel_ingenuo_lan_do_servidor_ping": E['RED_LAN_SERVIDOR'] + "/3", "tunel_ingenuo_lan_do_servidor_tcp": E['RED_LAN_SERVIDOR_TCP']},
  "block_local": {"resposta": j(E['R_LOCAL']), "impressora_de_casa": E['LOCAL_IMPRESSORA'] + "/3",
    "gateway_de_casa": E['LOCAL_GATEWAY'] + "/3", "site_viu": E['LOCAL_VISTO']},
  "ipv6_kernel_sem_ipv6": {"perfil_sem_a_marca_morreu": E['SEM_MARCA'] == "1", "log_linux_cant_add_ipv6": int(E['LOG_FATAL'] or 0)},
  "dns_empresa_sem_nomes": {"resposta": j(E['R_EMPRESA']), "site.externo.teste": E['EMPRESA_EXTERNO'],
    "dns_externo_viu_com_tunel_total": [x for x in E['EMPRESA_VIU_TT'].split(";") if x],
    "dns_externo_viu_sem_tunel_total": [x for x in E['EMPRESA_VIU_SEM_TT'].split(";") if x],
    "ana.matriz.phx_sem_os_nomes": E['EMPRESA_NOME_ANA']},
  "fim": {"resposta": j(E['R_DESLIGA']), "site_viu": E['FIM_VISTO'], "resolv_conf": E['FIM_RESOLV'], "ana.matriz.phx": E['FIM_NOME']},
  "recusas_pela_api": {"tunel_total_sem_dns": j(E['RECUSA_SEM_DNS']), "block_local_sem_tunel_total": j(E['RECUSA_LOCAL_SO']),
    "dns_privado_sem_rota": j(E['RECUSA_DNS_PRIVADO']), "dona_nao_admin": j(E['RECUSA_NAO_ADMIN']),
    "dns_linux_injetado": j(E['RECUSA_DNS_LINUX'])},
}
st, ct, d = r["sem_tunel_total"], r["com_tunel_total"], r["dns_nomes"]
ok = (st["site_viu"] == "192.168.50.11" and st["syn_na_wan_do_servidor"] == 0 and st["syn_na_casa"] >= 1
  and st["impressora_de_casa"] == "3/3" and st["ana.matriz.phx"] == "nao"
  and ct["site_viu"] == "192.0.2.1" and ct["syn_na_wan_do_servidor"] >= 1 and ct["syn_na_casa"] == 0
  and ct["impressora_de_casa_sem_block_local"] == "3/3"
  and ct["lan_do_servidor_sem_rota_ping"] == "0/3" and ct["lan_do_servidor_sem_rota_tcp"] == "falhou"
  and ct["membro_de_outra_rede"] == "0/3" and ct["log_block_outside_dns_recusado_e_seguiu"] >= 1
  and ct["log_ifconfig_ipv6_filtrado"] >= 1
  and d["ana.matriz.phx"] == d["ip_da_ana"] and d["ana_nome_curto_pela_search"] == d["ip_da_ana"]
  and d["xavier.matriz.phx_de_outra_rede"] == "nao" and d["site.externo.teste"] == "198.51.100.10"
  and "192.0.2.1 site.externo.teste" in d["dns_externo_viu"]
  and all(x.startswith("192.0.2.1 ") for x in d["dns_externo_viu"])
  and d["xavier_ping_ao_resolvedor_da_matriz"] == "3/3"
  and d["xavier_pergunta_ao_resolvedor_da_matriz"] == "sem resposta" and d["ana_pergunta_direto"] == d["ip_da_ana"]
  and r["RED"]["sem_nat_de_saida_site"] == "falhou" and r["RED"]["tunel_ingenuo_lan_do_servidor_ping"] == "3/3"
  and r["block_local"]["impressora_de_casa"] == "0/3" and r["block_local"]["gateway_de_casa"] == "3/3"
  and r["block_local"]["site_viu"] == "192.0.2.1"
  and r["ipv6_kernel_sem_ipv6"]["perfil_sem_a_marca_morreu"] and r["ipv6_kernel_sem_ipv6"]["log_linux_cant_add_ipv6"] >= 1
  and r["dns_empresa_sem_nomes"]["site.externo.teste"] == "198.51.100.10"
  and r["dns_empresa_sem_nomes"]["dns_externo_viu_com_tunel_total"] == ["192.0.2.1"]
  and r["dns_empresa_sem_nomes"]["dns_externo_viu_sem_tunel_total"] == ["192.168.50.11"]
  and r["dns_empresa_sem_nomes"]["ana.matriz.phx_sem_os_nomes"] == "nao"
  and r["fim"]["site_viu"] == "192.168.50.11"
  and r["regras"]["depois_de_desligar"] == r["regras"]["antes"]
  and r["regras"]["com_tunel_total"]["ip_forward"] == 1
  and r["perfil_da_ana_linhas_pedidas"] == 3
  and all(isinstance(v, dict) and "erro" in v for v in r["recusas_pela_api"].values()))
r["passou"] = ok
tudo = json.load(open(caminho)) if os.path.exists(caminho) else {}
tudo.update({"prova": "phxvpn: tunel total e DNS da rede (saida.rs, dns.rs), openvpn 2.6.19 em netns",
  "openvpn": "2.6.19", "kernel": "ipv6.disable=1 (sem IPv6: o block-ipv6 nao se prova em trafego aqui)", "n": 1})
tudo[motor] = r
json.dump(tudo, open(caminho, "w"), ensure_ascii=False, indent=2); open(caminho, "a").write("\n")
print("== PASSOU" if ok else "== FALHOU", motor)
sys.exit(0 if ok else 1)
PY
