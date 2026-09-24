#!/usr/bin/env bash
# Prova das redes alcancaveis (lacunas, itens 4 e 9) com o openvpn 2.6 DE
# VERDADE, painel + PostgreSQL + supervisor, em netns proprios:
#
#   S  servidor (painel, PostgreSQL, os openvpn das redes)
#   H  host da LAN da empresa ATRAS do servidor: 192.168.10.5 (sem rota
#      padrao -- so o NAT o faz responder a quem vem da VPN)
#   A  membro «ana» da rede Matriz
#   F  membro «filial» da Matriz: roteador da filial 192.168.20.0/24
#   FH host da filial: 192.168.20.5 (rota padrao pelo F)
#   X  membro «xavier» da rede Outra (a do isolamento)
#
# Casos, cada um nos dois sentidos (sem a configuracao nao alcanca; com, alcanca):
#   4   sem rota: A nao alcanca H (nem com rota manual: o servidor nao encaminha)
#   4   rota 192.168.10.0/24 com NAT: A alcanca H (ping e TCP), H ve o IP do servidor
#   4   isolamento: com o encaminhamento aceso, X (rede Outra) NAO alcanca A;
#       RED: sem a tabela de guarda, alcanca -- o ip_forward sozinho abre
#   4   «rota de volta»: sem NAT, A so alcanca H depois da rota no roteador
#       da empresa, e H ve o IP verdadeiro do membro
#   9   sem iroute: FH nao alcanca H; com a filial atras da «filial»: FH <-> H
#       (ping e TCP nos dois sentidos) e A -> FH
#   fim remover as rotas: o NAT e os accept somem, fica so a guarda entre as
#       redes (a tabela so some sem rede nenhuma: guarda.sh), as regras voltam
#       ao numero de antes e o ip_forward volta ao que era
#
# Uso (root):  MOTOR=nft ./rodar.sh       (padrao)
#              MOTOR=iptables ./rodar.sh
# Grava a chave do motor em resultados.json, ao lado.
set -euo pipefail

AQUI=$(cd "$(dirname "$0")" && pwd)
RAIZ=$(cd "$AQUI/../.." && pwd)
MOTOR=${MOTOR:-nft}
T=${1:-$(mktemp -d /tmp/phxvpn-prova-rotas.XXXX)}
BIN=${PHXVPN_BIN:-$RAIZ/target/debug/phxvpn}
PGBIN=$(ls -d /usr/lib/postgresql/*/bin | sort -V | tail -1)
PGPORTA=55493
PAINEL=127.0.0.1:8481
P=pxrt
NSS="$P-s $P-h $P-a $P-f $P-fh $P-x"
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
par s sa0 192.168.89.1/24 a eth0 192.168.89.11/24
par s sf0 192.168.88.1/24 f eth0 192.168.88.11/24
par s sx0 192.168.87.1/24 x eth0 192.168.87.11/24
par s lan 192.168.10.1/24 h eth0 192.168.10.5/24
par f lan 192.168.20.1/24 fh eth0 192.168.20.5/24
ns fh ip route add default via 192.168.20.1
# O roteador da filial encaminha (isso e do dono da filial, nao do painel).
echo 1 > /dev/null; ns f sh -c 'echo 1 > /proc/sys/net/ipv4/ip_forward'
for n in a:192.168.89.1 f:192.168.88.1 x:192.168.87.1; do
  mkdir -p /etc/netns/$P-${n%%:*}; echo "${n#*:} vpn.prova.local" > /etc/netns/$P-${n%%:*}/hosts
done

# --- PostgreSQL descartavel no netns do servidor
mkdir -p "$T/pg" "$T/pgsock"; chown postgres "$T/pg" "$T/pgsock"
echo "senha-pg-prova" > "$T/pgsenha"; chown postgres "$T/pgsenha"
su postgres -c "$PGBIN/initdb -D $T/pg -A scram-sha-256 --pwfile=$T/pgsenha -U postgres" >/dev/null
ns s su postgres -c "$PGBIN/pg_ctl -D $T/pg -o '-p $PGPORTA -k $T/pgsock -c listen_addresses=127.0.0.1' -l $T/pg/pg.log start" >/dev/null
sleep 2

PHXVPN_FIREWALL=$MOTOR PHXVPN_PG="host=127.0.0.1 port=$PGPORTA user=postgres password=senha-pg-prova dbname=postgres" \
  ns s "$BIN" painel --dados "$T/dados" --openvpn --escutar $PAINEL >"$T/painel.log" 2>&1 &
for _ in $(seq 50); do grep -q "CODIGO DE INSTALACAO" "$T/painel.log" && break; sleep 0.2; done
CODIGO=$(grep -o "CODIGO DE INSTALACAO: [^ ]*" "$T/painel.log" | awk '{print $4}')

api() { # api METODO CAMINHO TOKEN JSON -> corpo (erro: {"erro":...}, status 1)
  ns s python3 - "$@" <<'PY'
import json, sys, urllib.request
m, c, tk, corpo = sys.argv[1:5]
r = urllib.request.Request("http://127.0.0.1:8481" + c, method=m,
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
 \"servidor_nome\":\"vpn.prova.local\",\"servidor_ip\":\"192.168.89.1\",\"servidor_dns\":\"vpn.prova.local\",\"certificado_pem\":\"\"}" >/dev/null
TK=$(api POST /api/login "" '{"usuario":"admin","senha":"senha-admin-longa"}' | campo "['token']")
api POST /api/redes "$TK" '{"nome":"Matriz","senha":"senha-da-matriz","finalidade":"prova"}' >/dev/null
api POST /api/redes "$TK" '{"nome":"Outra","senha":"senha-da-outra","finalidade":"isolamento"}' >/dev/null
membro() { # LOGIN REDE SENHA_REDE -> token; grava o perfil
  api POST /api/usuarios "$TK" "{\"login\":\"$1\",\"senha\":\"senha-de-$1-longa\"}" >/dev/null
  local tk; tk=$(api POST /api/login "" "{\"usuario\":\"$1\",\"senha\":\"senha-de-$1-longa\"}" | campo "['token']")
  api POST /api/redes/entrar "$tk" "{\"nome\":\"$2\",\"senha\":\"$3\"}" | campo "['perfil']" > "$T/$1.ovpn"
  echo "$tk"
}
TK_ANA=$(membro ana Matriz senha-da-matriz)
membro filial Matriz senha-da-matriz >/dev/null
membro xavier Outra senha-da-outra >/dev/null
MATRIZ=$(api GET /api/redes "$TK" "" | python3 -c "import json,sys; print([r['id'] for r in json.load(sys.stdin) if r['nome']=='Matriz'][0])")
sleep 1

conectar() { # NS LOGIN -> espera o tunel
  local log="$T/c-$2.log"
  [ -f "$T/c-$2.pid" ] && { kill "$(cat "$T/c-$2.pid")" 2>/dev/null || true; sleep 1; }
  ns "$1" openvpn --config "$T/$2.ovpn" --daemon --log "$log" --writepid "$T/c-$2.pid"
  for _ in $(seq 100); do grep -q "Initialization Sequence Completed" "$log" 2>/dev/null && return 0; sleep 0.2; done
  echo "FALHOU: $2 nao conectou"; tail -5 "$log"; exit 1
}
ip_tun() { ns "$1" ip -4 -o addr show dev tun0 | awk '{print $4}' | cut -d/ -f1; }
pings() { # NS DESTINO -> recebidos de 3
  local n; n=$(ns "$1" ping -c 3 -W 1 -i 0.3 "$2" 2>/dev/null | awk '/received/ {print $4}' || true)
  echo "${n:-0}"
}
tcp() { # NS DESTINO -> o IP de quem conectou, visto pelo servidor de eco; ou "falhou"
  ns "$1" python3 -c "
import socket,sys
try:
    s=socket.create_connection(('$2',9000),timeout=3); print(s.recv(64).decode() or 'vazio')
except Exception as e: print('falhou')"
}
eco() { # NS: servidor TCP que devolve o IP de quem conectou
  ns "$1" python3 -c "
import socket
s=socket.socket(); s.setsockopt(socket.SOL_SOCKET,socket.SO_REUSEADDR,1); s.bind(('0.0.0.0',9000)); s.listen()
while True:
    c,a=s.accept(); c.sendall(a[0].encode()); c.close()" &
}
eco h; eco fh
regras() { # regras NOSSAS no kernel do servidor, e o total do ruleset (qualquer dono)
  local nossas total
  if [ "$MOTOR" = nft ]; then
    nossas=$(ns s nft -a list table ip phxvpn 2>/dev/null | grep '# handle' | grep -vc -e 'chain ' -e 'table ' || true)
  else
    nossas=$( { ns s iptables -S PHXVPN-ENC 2>/dev/null; ns s iptables -t nat -S PHXVPN-NAT 2>/dev/null; } | grep -c '^-A' || true)
  fi
  total=$(ns s nft -a list ruleset 2>/dev/null | grep -c '# handle' || true)
  # Regras de QUALQUER dono pelo iptables. O iptables-nft deixa as tabelas
  # filter/nat vazias que ele mesmo criou (handles, nao regras): por isso o
  # motor iptables compara regras, e o nft compara handles.
  local ipt; ipt=$(ns s iptables-save 2>/dev/null | grep -c '^-A' || true)
  echo "{\"nossas\": ${nossas:-0}, \"handles_no_ruleset\": ${total:-0}, \"regras_iptables_save\": ${ipt:-0}, \"ip_forward\": $(ns s cat /proc/sys/net/ipv4/ip_forward)}"
}
rota() { # incluir|remover CORPO_JSON -> resposta
  api POST "/api/redes/rotas/$1" "$TK" "{\"rede_id\":$MATRIZ,$2}" || true
}

# ----------------------------------------------------------- item 4 ----
ANTES=$(regras); echo "== regras antes: $ANTES"
conectar a ana
SEM_ROTA=$(pings a 192.168.10.5)
ns a ip route add 192.168.10.0/24 dev tun0
SEM_ROTA_MANUAL=$(pings a 192.168.10.5)
ns a ip route del 192.168.10.0/24 dev tun0
echo "== sem rota: A->H $SEM_ROTA/3; com rota manual no membro: $SEM_ROTA_MANUAL/3"

R_INC=$(rota incluir '"cidr":"192.168.10.0/24","volta":"nat"'); echo "== incluir NAT: $R_INC"
COM_NAT=$(regras)
conectar a ana
ROTA_EMPURRADA=$(ns a ip route show 192.168.10.0/24 | grep -c tun0 || true)
NAT_PING=$(pings a 192.168.10.5); NAT_TCP=$(tcp a 192.168.10.5)
echo "== com NAT: rota empurrada=$ROTA_EMPURRADA ping=$NAT_PING/3 tcp: H viu $NAT_TCP; regras $COM_NAT"

# Isolamento: X (Outra) com rota manual para a Matriz, A com rota de volta.
conectar x xavier
IP_ANA=$(ip_tun a)
ns x ip route add 10.77.$(ip_tun a | cut -d. -f3).0/24 dev tun0
ns a ip route add "$(ip_tun x | cut -d. -f1-3).0/24" dev tun0
ISO_GUARDA=$(pings x "$IP_ANA")
if [ "$MOTOR" = nft ]; then ns s nft delete table ip phxvpn; else
  ns s iptables -D FORWARD -j PHXVPN-ENC; fi
ISO_RED=$(pings x "$IP_ANA")
echo "== isolamento X(Outra)->A(Matriz): com a guarda $ISO_GUARDA/3; RED sem a guarda (ip_forward=1) $ISO_RED/3"
ns a ip route del "$(ip_tun x | cut -d. -f1-3).0/24" dev tun0

# Rota de volta (sem NAT). Remover + incluir reaplica o firewall inteiro.
rota remover '"cidr":"192.168.10.0/24"' >/dev/null
R_VOLTA=$(rota incluir '"cidr":"192.168.10.0/24","volta":"rota"')
conectar a ana
VOLTA_SEM=$(tcp a 192.168.10.5)
ns h ip route add 10.77.0.0/16 via 192.168.10.1
VOLTA_COM=$(tcp a 192.168.10.5)
echo "== rota de volta: sem a rota no roteador H viu '$VOLTA_SEM'; com: '$VOLTA_COM' (IP da ana $(ip_tun a))"
ns h ip route del 10.77.0.0/16 via 192.168.10.1
rota remover '"cidr":"192.168.10.0/24"' >/dev/null
rota incluir '"cidr":"192.168.10.0/24","volta":"nat"' >/dev/null

# ----------------------------------------------------------- item 9 ----
conectar f filial
conectar a ana
SITE_SEM=$(pings fh 192.168.10.5)
echo "== sem iroute: FH->H $SITE_SEM/3"
NAO_ADMIN=$(api POST /api/redes/rotas/incluir "$TK_ANA" "{\"rede_id\":$MATRIZ,\"cidr\":\"192.168.30.0/24\"}" || true)
R_SITE=$(rota incluir '"cidr":"192.168.20.0/24","membro":"filial"'); echo "== incluir filial: $R_SITE"
COM_SITE=$(regras)
conectar f filial
conectar a ana
ns h ip route add 192.168.20.0/24 via 192.168.10.1
F_ROTA_PROPRIA=$(ns f ip route show 192.168.20.0/24 | grep -c tun0 || true)
A_ROTA_SITE=$(ns a ip route show 192.168.20.0/24 | grep -c tun0 || true)
S_ROTA_SITE=$(ns s ip route show 192.168.20.0/24 | grep -c tun || true)
FH_H_PING=$(pings fh 192.168.10.5); FH_H_TCP=$(tcp fh 192.168.10.5)
H_FH_PING=$(pings h 192.168.20.5); H_FH_TCP=$(tcp h 192.168.20.5)
A_FH_PING=$(pings a 192.168.20.5); A_FH_TCP=$(tcp a 192.168.20.5)
echo "== filial: FH->H $FH_H_PING/3 ($FH_H_TCP); H->FH $H_FH_PING/3 ($H_FH_TCP); A->FH $A_FH_PING/3 ($A_FH_TCP)"
echo "== rotas: F tem a propria pelo tunel=$F_ROTA_PROPRIA (0 = push-remove); A=$A_ROTA_SITE; kernel do S=$S_ROTA_SITE"
TIRAR_FILIAL=$(api POST /api/redes/remover "$TK" "{\"rede_id\":$MATRIZ,\"login\":\"filial\"}" || true)

# Recusas pela API (a validacao no caminho real).
RECUSA_00=$(rota incluir '"cidr":"0.0.0.0/0"'); RECUSA_VPN=$(rota incluir '"cidr":"10.77.0.0/16"')
RECUSA_HOST=$(rota incluir '"cidr":"192.168.10.5/24"'); RECUSA_PUB=$(rota incluir '"cidr":"8.8.8.0/24"')
RECUSA_SOBRE=$(rota incluir '"cidr":"192.168.20.128/25"')
CONF="$T/dados/redes/$MATRIZ/servidor.conf"
CONF_ROTAS=$(grep -e '^route ' -e '^push "route' "$CONF" || true)
CCD_FILIAL=$(cat "$T"/dados/redes/$MATRIZ/ccd/filial.* )

# ------------------------------------------------------------- fim ----
rota remover '"cidr":"192.168.20.0/24"' >/dev/null
rota remover '"cidr":"192.168.10.0/24"' >/dev/null
DEPOIS=$(regras); echo "== regras depois de remover tudo: $DEPOIS"
conectar a ana
FIM_PING=$(pings a 192.168.10.5)
TABELA_FICOU=$(ns s nft list tables 2>/dev/null | grep -c 'ip phxvpn' || true)

export ANTES A_FH_PING A_FH_TCP A_ROTA_SITE CCD_FILIAL COM_NAT COM_SITE CONF_ROTAS DEPOIS FH_H_PING FH_H_TCP FIM_PING F_ROTA_PROPRIA H_FH_PING H_FH_TCP ISO_GUARDA ISO_RED NAO_ADMIN NAT_PING NAT_TCP RECUSA_00 RECUSA_HOST RECUSA_PUB RECUSA_SOBRE RECUSA_VPN ROTA_EMPURRADA R_INC R_SITE SEM_ROTA SEM_ROTA_MANUAL SITE_SEM S_ROTA_SITE TABELA_FICOU TIRAR_FILIAL VOLTA_COM VOLTA_SEM
python3 - "$AQUI/resultados.json" "$MOTOR" <<'PY'
import json, sys, os, datetime
caminho, motor = sys.argv[1], sys.argv[2]
E = os.environ
j = lambda t: json.loads(t) if t.strip().startswith("{") else t
r = {
  "data": datetime.datetime.now(datetime.timezone.utc).isoformat(timespec="seconds"),
  "regras_antes": j(E['ANTES']), "regras_com_nat": j(E['COM_NAT']),
  "regras_com_filial": j(E['COM_SITE']), "regras_depois_de_remover": j(E['DEPOIS']),
  "tabela_phxvpn_depois": int(E['TABELA_FICOU'] or 0),
  "item4": {
    "sem_rota_ping": E['SEM_ROTA'] + '/3', "sem_rota_com_rota_manual_no_membro": E['SEM_ROTA_MANUAL'] + '/3',
    "resposta_incluir": j(E['R_INC']), "rota_empurrada_ao_membro": int(E['ROTA_EMPURRADA'] or 0),
    "nat_ping": E['NAT_PING'] + '/3', "nat_tcp_h_viu": E['NAT_TCP'],
    "isolamento_outra_rede_com_guarda": E['ISO_GUARDA'] + '/3', "isolamento_RED_sem_guarda": E['ISO_RED'] + '/3',
    "rota_de_volta_sem_rota_no_roteador": E['VOLTA_SEM'], "rota_de_volta_com_rota_no_roteador_h_viu": E['VOLTA_COM'],
    "depois_de_remover_ping": E['FIM_PING'] + '/3',
  },
  "item9": {
    "sem_iroute_fh_h_ping": E['SITE_SEM'] + '/3', "resposta_incluir": j(E['R_SITE']),
    "fh_h_ping": E['FH_H_PING'] + '/3', "fh_h_tcp_h_viu": E['FH_H_TCP'],
    "h_fh_ping": E['H_FH_PING'] + '/3', "h_fh_tcp_fh_viu": E['H_FH_TCP'],
    "a_fh_ping": E['A_FH_PING'] + '/3', "a_fh_tcp_fh_viu": E['A_FH_TCP'],
    "filial_recebe_rota_da_propria_lan": int(E['F_ROTA_PROPRIA'] or 0), "ana_recebe_rota_da_filial": int(E['A_ROTA_SITE'] or 0),
    "kernel_do_servidor_tem_rota_da_filial": int(E['S_ROTA_SITE'] or 0),
    "tirar_membro_com_filial": j(E['TIRAR_FILIAL']),
  },
  "recusas_pela_api": {"0.0.0.0/0": j(E['RECUSA_00']), "10.77.0.0/16": j(E['RECUSA_VPN']),
    "192.168.10.5/24": j(E['RECUSA_HOST']), "8.8.8.0/24": j(E['RECUSA_PUB']),
    "192.168.20.128/25": j(E['RECUSA_SOBRE']), "nao_admin": j(E['NAO_ADMIN'])},
  "conf_do_servidor": E['CONF_ROTAS'].splitlines(), "ccd_da_filial": E['CCD_FILIAL'].splitlines(),
}
ok = (r["item4"]["sem_rota_ping"] == "0/3" and r["item4"]["sem_rota_com_rota_manual_no_membro"] == "0/3"
  and r["item4"]["nat_ping"] == "3/3" and r["item4"]["nat_tcp_h_viu"] == "192.168.10.1"
  and r["item4"]["isolamento_outra_rede_com_guarda"] == "0/3" and r["item4"]["isolamento_RED_sem_guarda"] != "0/3"
  and r["item4"]["rota_de_volta_sem_rota_no_roteador"] == "falhou"
  and r["item4"]["rota_de_volta_com_rota_no_roteador_h_viu"].startswith("10.77.")
  and r["item4"]["depois_de_remover_ping"] == "0/3"
  and r["item9"]["sem_iroute_fh_h_ping"] == "0/3" and r["item9"]["fh_h_ping"] == "3/3"
  and r["item9"]["h_fh_ping"] == "3/3" and r["item9"]["a_fh_ping"] == "3/3"
  and r["item9"]["h_fh_tcp_fh_viu"] == "192.168.10.5" and r["item9"]["filial_recebe_rota_da_propria_lan"] == 0
  and r["regras_depois_de_remover"]["nossas"] == r["regras_antes"]["nossas"] and r["regras_depois_de_remover"]["ip_forward"] == 0
  and (r["regras_depois_de_remover"]["handles_no_ruleset"] == r["regras_antes"]["handles_no_ruleset"] if motor == "nft"
       else r["regras_depois_de_remover"]["regras_iptables_save"] == r["regras_antes"]["regras_iptables_save"])
  and r["tabela_phxvpn_depois"] == (1 if motor == "nft" else 0)
  and all(isinstance(v, dict) and "erro" in v for v in r["recusas_pela_api"].values())
  and isinstance(r["item9"]["tirar_membro_com_filial"], dict) and "erro" in r["item9"]["tirar_membro_com_filial"])
r["passou"] = ok
tudo = json.load(open(caminho)) if os.path.exists(caminho) else {"prova": "phxvpn: redes alcancaveis (itens 4 e 9), openvpn 2.6.19 em netns", "openvpn": "2.6.19"}
tudo[motor] = r
json.dump(tudo, open(caminho, "w"), ensure_ascii=False, indent=2); open(caminho, "a").write("\n")
print("== PASSOU" if ok else "== FALHOU", motor)
sys.exit(0 if ok else 1)
PY
