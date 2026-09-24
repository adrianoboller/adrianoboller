#!/usr/bin/env bash
# Prova com o openvpn 2.6.19 de verdade, em netns, dos itens 10 e 8 das
# lacunas e do `mlock` do modo servidor:
#
#   historico -- dois membros conectam, um trafega e sai: a tabela
#                phx_conexao tem uma linha por sessao (IP real, IP da VPN,
#                entrou, saiu, bytes); o admin ve as duas, a ana so a dela;
#                nenhuma senha em lugar nenhum (banco, log do painel, log do
#                openvpn).
#   cookie    -- rede v2 em UDP: o cliente 2.6.19 entra; o cliente 2.5.11
#                (compilado do fonte, sem suporte ao cookie) NAO entra com
#                `force-cookie` e entra sem ele (o binario de antes).
#   mlock     -- VmLck do painel e do openvpn filho no /proc.
#   ponte     -- (so o binario novo) rede UDP com queda para TCP 443 e o UDP
#                do caio bloqueado: ele entra pela ponte, o openvpn o ve como
#                127.x.y.z, e o historico tem de gravar o IP de FORA -- com o
#                `force-cookie` ligado (o pacote chega ao openvpn por UDP).
#
# Roda o MESMO roteiro com o binario novo (VERDE) e o de antes (VELHO, o RED).
# Topologia: netns do servidor (PostgreSQL, painel, openvpn) com uma ponte e
# um netns por membro. Nenhuma porta do hospedeiro; processos morrem por PID.
#
# Uso: sudo PHXVPN_BIN_VELHO=... PHXVPN_OVPN25=.../openvpn ./openvpn.sh
set -euo pipefail
[ -n "${PHXVPN_PROVA_X:-}" ] && set -x

AQUI=$(cd "$(dirname "$0")" && pwd)
RAIZ=$(cd "$AQUI/../.." && pwd)
BIN_NOVO=${PHXVPN_BIN:-$RAIZ/target/debug/phxvpn}
BIN_VELHO=${PHXVPN_BIN_VELHO:-}
OVPN25=${PHXVPN_OVPN25:-}
PGBIN=$(ls -d /usr/lib/postgresql/*/bin | sort -V | tail -1)
PGPORTA=55462
PAINEL=127.0.0.1:8487
NS_S=pxop-s; NS_A=pxop-a; NS_B=pxop-b; NS_C=pxop-c

limpar() {
  set +e
  for n in $NS_A $NS_B $NS_C $NS_S; do ip netns pids $n 2>/dev/null | xargs -r kill; done
  sleep 1
  [ -n "${T:-}" ] && [ -d "$T/pg" ] && ip netns exec $NS_S su postgres -c "$PGBIN/pg_ctl -D $T/pg -m fast stop" >/dev/null 2>&1
  for n in $NS_A $NS_B $NS_C $NS_S; do ip netns pids $n 2>/dev/null | xargs -r kill -9; ip netns del $n 2>/dev/null; rm -rf /etc/netns/$n; done
}
S() { ip netns exec $NS_S "$@"; }

rodada() { # rodada ROTULO BINARIO -> uma linha JSON
  local rotulo=$1 bin=$2
  T=$(mktemp -d /tmp/phxvpn-prova-op.XXXX); chmod 755 "$T"
  echo "== [$rotulo] pasta: $T ($bin)" >&2
  limpar
  set -e
  ip netns add $NS_S
  S ip link set lo up
  S ip link add pxobr type bridge; S ip addr add 192.168.93.1/24 dev pxobr; S ip link set pxobr up
  local i=1
  for n in $NS_A $NS_B $NS_C; do
    ip netns add $n; ip -n $n link set lo up
    ip link add pxov$i netns $NS_S type veth peer name eth0 netns $n
    S ip link set pxov$i master pxobr up
    ip -n $n addr add 192.168.93.1$i/24 dev eth0; ip -n $n link set eth0 up
    mkdir -p /etc/netns/$n; echo "192.168.93.1 vpn.prova.local" > /etc/netns/$n/hosts
    i=$((i + 1))
  done

  mkdir -p "$T/pg" "$T/pgsock"; chown postgres "$T/pg" "$T/pgsock"
  echo "senha-pg-prova" > "$T/pgsenha"; chown postgres "$T/pgsenha"
  su postgres -c "$PGBIN/initdb -D $T/pg -A scram-sha-256 --pwfile=$T/pgsenha -U postgres" >/dev/null
  S su postgres -c "$PGBIN/pg_ctl -D $T/pg -o '-p $PGPORTA -k $T/pgsock -c listen_addresses=127.0.0.1' -l $T/pg/pg.log start" >/dev/null
  sleep 2

  ip netns exec $NS_S env \
    PHXVPN_PG="host=127.0.0.1 port=$PGPORTA user=postgres password=senha-pg-prova dbname=postgres" \
    "$bin" painel --dados "$T/dados" --openvpn --escutar $PAINEL >"$T/painel.log" 2>&1 &
  local pid_painel=$!
  for _ in $(seq 50); do grep -q "CODIGO DE INSTALACAO" "$T/painel.log" && break; sleep 0.2; done
  local codigo; codigo=$(grep -o "CODIGO DE INSTALACAO: [^ ]*" "$T/painel.log" | awk '{print $4}')

  api() {
    S python3 - "$@" <<'PY'
import json, sys, urllib.request
m, c, tk, corpo = sys.argv[1:5]
r = urllib.request.Request("http://127.0.0.1:8487" + c, method=m,
    data=corpo.encode() if m == "POST" else None,
    headers={"Content-Type": "application/json", **({"Authorization": "Bearer " + tk} if tk else {})})
try:
    print(urllib.request.urlopen(r).read().decode())
except urllib.error.HTTPError as e:
    print(json.dumps({"status": e.code, "corpo": e.read().decode()}))
PY
  }
  campo() { python3 -c "import json,sys; print(json.load(sys.stdin)['$1'])"; }
  api POST /api/instalar "" "{\"codigo_instalacao\":\"$codigo\",\"empresa\":\"Prova Ltda\",\"finalidade\":\"prova\",
 \"responsavel\":\"Prova\",\"email\":\"p@prova.local\",\"telefone\":\"0\",\"admin_usuario\":\"admin\",
 \"admin_senha\":\"senha-admin-longa\",\"senha_mestre\":\"senha-mestre-longa-da-prova\",
 \"servidor_nome\":\"vpn.prova.local\",\"servidor_ip\":\"192.168.93.1\",\"servidor_dns\":\"vpn.prova.local\",\"certificado_pem\":\"\"}" >/dev/null
  local tk_admin tk_ana tk_caio rede_id
  tk_admin=$(api POST /api/login "" '{"usuario":"admin","senha":"senha-admin-longa"}' | campo token)
  api POST /api/redes "$tk_admin" '{"nome":"Matriz","senha":"senha-da-rede","finalidade":"prova"}' | campo perfil > "$T/admin.ovpn"
  for u in ana caio; do
    api POST /api/usuarios "$tk_admin" "{\"login\":\"$u\",\"senha\":\"senha-da-$u-longa\",\"email\":\"$u@prova.local\"}" >/dev/null
    local tk; tk=$(api POST /api/login "" "{\"usuario\":\"$u\",\"senha\":\"senha-da-$u-longa\"}" | campo token)
    [ $u = ana ] && tk_ana=$tk
    [ $u = caio ] && tk_caio=$tk
    api POST /api/redes/entrar "$tk" '{"nome":"Matriz","senha":"senha-da-rede"}' | campo perfil > "$T/$u.ovpn"
  done
  rede_id=$(api GET /api/redes "$tk_admin" "" | python3 -c "import json,sys; print(json.load(sys.stdin)[0]['id'])")
  local dir="$T/dados/redes/$rede_id" status="$T/dados/redes/$rede_id/status.log"
  sleep 1

  sobe() { # NETNS PERFIL LOG [BINARIO]
    ip netns exec "$1" "${4:-openvpn}" --config "$2" --log "$3" --daemon --writepid "$3.pid"
  }
  completos() { grep -c "Initialization Sequence Completed" "$1" 2>/dev/null || true; }
  na_lista() { grep -c "^CLIENT_LIST," "$status" 2>/dev/null || true; }
  esperar() { # SEGUNDOS COMANDO... -> 0 se deu certo no prazo
    local fim=$(( $(date +%s) + $1 )); shift
    while [ "$(date +%s)" -lt "$fim" ]; do "$@" && return 0; sleep 0.2; done
    return 1
  }
  lista_tem() { [ "$(na_lista)" -eq "$1" ]; }
  entrou() { [ "$(completos "$1")" -gt 0 ]; }

  local conf_cookie=0 conf_hist=0 conf_mlock=0
  grep -q "^tls-crypt-v2 .* force-cookie$" "$dir/servidor.conf" && conf_cookie=1
  grep -q "^client-connect " "$dir/servidor.conf" && conf_hist=1
  grep -q "^mlock$" "$dir/servidor.conf" && conf_mlock=1

  # --- historico: admin (A) e ana (B) conectam; a ana trafega e sai.
  sobe $NS_A "$T/admin.ovpn" "$T/a.log"; sobe $NS_B "$T/ana.ovpn" "$T/b.log"
  esperar 60 lista_tem 2 || true
  ip netns exec $NS_B ping -c 20 -i 0.2 -s 1000 -W 1 10.77.$rede_id.1 >/dev/null 2>&1 || true
  kill -TERM "$(cat "$T/b.log.pid")"
  esperar 30 lista_tem 1 || true
  sleep 3
  api GET /api/historico "$tk_admin" "" > "$T/hist-admin.json"
  api GET /api/historico "$tk_ana" "" > "$T/hist-ana.json"
  local linhas_banco
  linhas_banco=$(S env PGPASSWORD=senha-pg-prova "$PGBIN/psql" -h 127.0.0.1 -p $PGPORTA -U postgres -At \
    -c "SELECT count(*) FROM phx_conexao" 2>/dev/null || echo null)
  S env PGPASSWORD=senha-pg-prova "$PGBIN/pg_dump" -h 127.0.0.1 -p $PGPORTA -U postgres -t 'phx_conexao*' \
    postgres > "$T/dump.sql" 2>/dev/null || : > "$T/dump.sql"
  local vazou=0
  for s in senha-admin-longa senha-da-ana-longa senha-da-caio-longa senha-da-rede senha-mestre-longa-da-prova; do
    for f in "$T/dump.sql" "$T/painel.log" "$dir"/openvpn.log*; do
      [ -f "$f" ] && grep -q -- "$s" "$f" && vazou=$((vazou + 1))
    done
  done

  # --- mlock: VmLck do painel e do openvpn filho.
  local pid_ovpn vmlck_painel vmlck_ovpn
  pid_ovpn=$(pgrep -P "$pid_painel" -x openvpn || true)
  vmlck_painel=$(awk '/^VmLck:/ {print $2}' /proc/$pid_painel/status)
  vmlck_ovpn=$( [ -n "$pid_ovpn" ] && awk '/^VmLck:/ {print $2}' /proc/$pid_ovpn/status || echo 0)

  # --- cookie: o 2.6.19 ja entrou (A); o 2.5.11 tenta como caio (C).
  local c25=null
  if [ -n "$OVPN25" ]; then
    sobe $NS_C "$T/caio.ovpn" "$T/c.log" "$OVPN25"
    if esperar 25 entrou "$T/c.log"; then c25=1; else c25=0; fi
    kill -TERM "$(cat "$T/c.log.pid")" 2>/dev/null || true
  fi
  local a26=0; entrou "$T/a.log" && a26=1

  # --- ponte: queda para TCP 443, UDP do caio bloqueado, cliente 2.6.19.
  local ponte_entrou=null ponte_linha=null
  local gravou; gravou=$(api POST /api/redes/alcance/gravar "$tk_admin" "{\"rede_id\":$rede_id,\"queda_tcp\":443}")
  if ! echo "$gravou" | grep -q '"status"'; then
    S iptables -A INPUT -s 192.168.93.13 -p udp -j DROP
    sleep 3
    api POST /api/redes/entrar "$tk_caio" '{"nome":"Matriz","senha":"senha-da-rede"}' | campo perfil > "$T/caio-tcp.ovpn"
    sobe $NS_C "$T/caio-tcp.ovpn" "$T/c2.log"
    if esperar 60 entrou "$T/c2.log"; then ponte_entrou=1; else ponte_entrou=0; fi
    sleep 2
    ip netns exec $NS_C ping -c 5 -i 0.2 -W 1 10.77.$rede_id.1 >/dev/null 2>&1 || true
    kill -TERM "$(cat "$T/c2.log.pid")" 2>/dev/null || true
    # O TCP fechado nao vira `explicit-exit-notify`: o openvpn so ve a saida
    # no `ping-restart` (ate ~120 s). A linha tem de fechar com o IP de fora
    # que a ponte lembrou -- e continuar UMA so.
    local t_ponte; t_ponte=$(date +%s)
    while :; do
      ponte_linha=$(api GET /api/historico "$tk_admin" "" | python3 -c "
import json, sys
l = [c for c in json.load(sys.stdin)['conexoes'] if c['login'] == 'caio']
print(json.dumps({**l[0], 'linhas_do_caio': len(l)} if l else None))")
      echo "$ponte_linha" | grep -q '"estado": "saiu"' && break
      [ $(( $(date +%s) - t_ponte )) -gt 180 ] && break
      sleep 5
    done
    S iptables -D INPUT -s 192.168.93.13 -p udp -j DROP
  fi

  python3 - "$rotulo" "$bin" "$T/hist-admin.json" "$T/hist-ana.json" "$linhas_banco" "$vazou" \
    "$conf_cookie" "$conf_hist" "$conf_mlock" "$vmlck_painel" "$vmlck_ovpn" "$a26" "$c25" \
    "$ponte_entrou" "$ponte_linha" <<'PY'
import json, sys
a = sys.argv[1:]
def carregar(f):
    try:
        return json.load(open(f))
    except Exception:
        return None
ha, hn = carregar(a[2]), carregar(a[3])
def linhas(h):
    return h.get("conexoes") if isinstance(h, dict) and "conexoes" in h else None
la, ln = linhas(ha), linhas(hn)
ana = [c for c in (la or []) if c.get("login") == "ana"]
num = lambda s: None if s == "null" else int(s)
print(json.dumps({
    "rodada": a[0], "binario": a[1],
    "conf_force_cookie": a[6] == "1", "conf_client_connect": a[7] == "1", "conf_mlock": a[8] == "1",
    "historico_http": "ok" if la is not None else (ha or {}).get("status"),
    "linhas_no_banco": num(a[4]),
    "linhas_vistas_pelo_admin": None if la is None else len(la),
    "linhas_vistas_pela_ana": None if ln is None else len(ln),
    "logins_vistos_pela_ana": None if ln is None else sorted({c["login"] for c in ln}),
    "ana": ana[0] if ana else None,
    "admin_estado": next((c["estado"] for c in (la or []) if c.get("login") == "admin"), None),
    "senhas_encontradas_em_banco_e_logs": int(a[5]),
    "vmlck_painel_kib": int(a[9] or 0), "vmlck_openvpn_kib": int(a[10] or 0),
    "cliente_26_entrou": a[11] == "1",
    "cliente_25_entrou": None if a[12] == "null" else a[12] == "1",
    "ponte_cliente_26_tcp_entrou": None if a[13] == "null" else a[13] == "1",
    "ponte_linha_do_caio": json.loads(a[14]),
}, ensure_ascii=False))
PY
  limpar
}

trap limpar EXIT
VERDE=null
[ -z "${PHXVPN_SO_VELHO:-}" ] && VERDE=$(rodada novo "$BIN_NOVO")
VELHO=null
[ -n "$BIN_VELHO" ] && VELHO=$(rodada velho "$BIN_VELHO")
# O que o `--mlock` do proprio openvpn faz sob o limite DESTE ambiente: e o
# RED da guarda `openvpn_aguenta_mlock` (escrever a diretiva sempre).
MLOCK_OVPN=$(cd /tmp && openvpn --mlock --genkey secret "$(mktemp -u /tmp/phxvpn-mlock.XXXX)" 2>&1 | tail -2 | tr '\n' ' ' || true)
python3 - "$VERDE" "$VELHO" "$AQUI" "$(openvpn --version | head -1 | awk '{print $2}')" "${OVPN25:+$($OVPN25 --version | head -1 | awk '{print $2}')}" \
  "$(ulimit -Sl)" "$(ulimit -Hl)" "$(awk '/^CapEff:/ {print $2}' /proc/self/status)" "$MLOCK_OVPN" <<'PY'
import json, sys, datetime, platform, subprocess
novo, velho = json.loads(sys.argv[1]), json.loads(sys.argv[2])
cap = int(sys.argv[8], 16)
ambiente = {
  "rlimit_memlock_macio": sys.argv[6], "rlimit_memlock_duro": sys.argv[7],
  "cap_ipc_lock": bool(cap >> 14 & 1), "cap_sys_resource": bool(cap >> 24 & 1),
  "openvpn_mlock_direto": sys.argv[9].strip(),
}
ambiente["openvpn_mlock_sobe_aqui"] = "fatal error" not in ambiente["openvpn_mlock_direto"]
ana = novo["ana"] or {}
ok = (novo["conf_force_cookie"] and novo["conf_client_connect"]
      and novo["linhas_no_banco"] == 2 and novo["linhas_vistas_pelo_admin"] == 2
      and novo["linhas_vistas_pela_ana"] == 1 and novo["logins_vistos_pela_ana"] == ["ana"]
      and ana.get("estado") == "saiu" and ana.get("ip_real") == "192.168.93.12"
      and (ana.get("bytes_do_membro") or 0) > 20000 and (ana.get("bytes_ao_membro") or 0) > 20000
      and novo["admin_estado"] == "conectado"
      and novo["senhas_encontradas_em_banco_e_logs"] == 0
      and novo["vmlck_painel_kib"] > 0
      # O `mlock` do openvpn so vai quando ele consegue subir o limite; aqui
      # (sem CAP_SYS_RESOURCE, limite duro 8 MiB) ele nao vai -- e a rede sobe.
      and novo["conf_mlock"] == (novo["vmlck_openvpn_kib"] > 0)
      and novo["conf_mlock"] == ambiente["openvpn_mlock_sobe_aqui"]
      and novo["cliente_26_entrou"] and novo["cliente_25_entrou"] is False
      and novo["ponte_cliente_26_tcp_entrou"] is True
      and (novo["ponte_linha_do_caio"] or {}).get("ip_real") == "192.168.93.13"
      and (novo["ponte_linha_do_caio"] or {}).get("pela_ponte") is True
      and (novo["ponte_linha_do_caio"] or {}).get("linhas_do_caio") == 1
      and (novo["ponte_linha_do_caio"] or {}).get("estado") == "saiu")
red = velho is None or (
      velho["cliente_26_entrou"]
      and not velho["conf_force_cookie"] and not velho["conf_client_connect"]
      and velho["historico_http"] == 404
      and velho["vmlck_painel_kib"] == 0 and velho["vmlck_openvpn_kib"] == 0
      and velho["cliente_25_entrou"] is True)
r = {
  "prova": "openvpn 2.6.19 em netns: historico de conexoes, tls-crypt-v2 force-cookie e mlock",
  "medido_em": datetime.datetime.now(datetime.timezone.utc).isoformat(timespec="seconds"),
  "kernel": platform.release(), "n": 1,
  "openvpn_servidor_e_cliente": sys.argv[4], "openvpn_cliente_antigo": sys.argv[5] or None,
  "ambiente": ambiente,
  "novo": novo, "velho_red": velho,
  "confere_verde": ok, "confere_red": red if velho is not None else None,
}
json.dump(r, open(sys.argv[3] + "/secao-openvpn.tmp.json", "w"), ensure_ascii=False, indent=1)
subprocess.run([sys.executable, sys.argv[3] + "/juntar.py", "openvpn", sys.argv[3] + "/secao-openvpn.tmp.json"], check=True)
import os; os.remove(sys.argv[3] + "/secao-openvpn.tmp.json")
print(json.dumps(r, ensure_ascii=False, indent=1))
sys.exit(0 if ok and (velho is None or red) else 1)
PY
