#!/usr/bin/env bash
# Prova do segundo fator na CONEXAO, com o openvpn 2.6 DE VERDADE:
# PostgreSQL -> painel -> ana cadastra o autenticador -> a rede passa a exigir
# -> ana conecta por usuario + senha + codigo (static-challenge / SCRV1).
#
# Casos: senha errada (com codigo valido), codigo certo, o MESMO codigo de
# novo, codigo errado. Depois o RED: com a conferencia trocada por /bin/true,
# a senha errada e o codigo errado CONECTAM -- isto e, a prova reprova quando
# a conferencia some.
#
# Tudo em dois netns proprios (servidor e cliente): nenhuma porta do
# hospedeiro, para rodar ao lado de outras provas. Processos morrem por PID.
#
# Uso:  sudo ./rodar.sh [pasta-de-trabalho]      (grava resultados.json aqui)
set -euo pipefail

AQUI=$(cd "$(dirname "$0")" && pwd)
RAIZ=$(cd "$AQUI/../.." && pwd)
T=${1:-$(mktemp -d /tmp/phxvpn-prova-mfa.XXXX)}
BIN=${PHXVPN_BIN:-$RAIZ/target/debug/phxvpn}
PGBIN=$(ls -d /usr/lib/postgresql/*/bin | sort -V | tail -1)
PGPORTA=55492
PAINEL=127.0.0.1:8479
NS_S=pxmfa-s; NS_C=pxmfa-c
mkdir -p "$T"; chmod 755 "$T"
echo "== pasta: $T"

limpar() {
  set +e
  for n in $NS_C $NS_S; do ip netns pids $n 2>/dev/null | xargs -r kill; done
  sleep 1
  [ -d "$T/pg" ] && ip netns exec $NS_S su postgres -c "$PGBIN/pg_ctl -D $T/pg -m fast stop" >/dev/null 2>&1
  for n in $NS_C $NS_S; do ip netns pids $n 2>/dev/null | xargs -r kill -9; ip netns del $n 2>/dev/null; rm -rf /etc/netns/$n; done
}
trap limpar EXIT
S() { ip netns exec $NS_S "$@"; }
C() { ip netns exec $NS_C "$@"; }

# --- dois netns ligados por um par veth
ip netns add $NS_S; ip netns add $NS_C
S ip link set lo up; C ip link set lo up
ip link add pxmfa0 netns $NS_S type veth peer name eth0 netns $NS_C
S ip addr add 192.168.89.1/24 dev pxmfa0; S ip link set pxmfa0 up
C ip addr add 192.168.89.11/24 dev eth0; C ip link set eth0 up
mkdir -p /etc/netns/$NS_C; echo "192.168.89.1 vpn.prova.local" > /etc/netns/$NS_C/hosts

# --- PostgreSQL descartavel, com SCRAM, dentro do netns do servidor
mkdir -p "$T/pg" "$T/pgsock"; chown postgres "$T/pg" "$T/pgsock"
echo "senha-pg-prova" > "$T/pgsenha"; chown postgres "$T/pgsenha"
su postgres -c "$PGBIN/initdb -D $T/pg -A scram-sha-256 --pwfile=$T/pgsenha -U postgres" >/dev/null
S su postgres -c "$PGBIN/pg_ctl -D $T/pg -o '-p $PGPORTA -k $T/pgsock -c listen_addresses=127.0.0.1' -l $T/pg/pg.log start" >/dev/null
sleep 2

# --- Painel com o supervisor do OpenVPN
PHXVPN_PG="host=127.0.0.1 port=$PGPORTA user=postgres password=senha-pg-prova dbname=postgres" \
  S "$BIN" painel --dados "$T/dados" --openvpn --escutar $PAINEL >"$T/painel.log" 2>&1 &
for _ in $(seq 50); do grep -q "CODIGO DE INSTALACAO" "$T/painel.log" && break; sleep 0.2; done
CODIGO=$(grep -o "CODIGO DE INSTALACAO: [^ ]*" "$T/painel.log" | awk '{print $4}')

api() { # api METODO CAMINHO TOKEN JSON
  S python3 - "$@" <<'PY'
import json, sys, urllib.request
m, c, tk, corpo = sys.argv[1:5]
r = urllib.request.Request("http://127.0.0.1:8479" + c, method=m,
    data=corpo.encode() if m == "POST" else None,
    headers={"Content-Type": "application/json", **({"Authorization": "Bearer " + tk} if tk else {})})
try:
    print(urllib.request.urlopen(r).read().decode())
except urllib.error.HTTPError as e:
    print(e.read().decode()); sys.exit(1)
PY
}
campo() { python3 -c "import json,sys; print(json.load(sys.stdin)['$1'])"; }
codigo() { python3 "$AQUI/cliente.py" --codigo "$SEGREDO" "$1"; }

api POST /api/instalar "" "{\"codigo_instalacao\":\"$CODIGO\",\"empresa\":\"Prova Ltda\",\"finalidade\":\"prova\",
 \"responsavel\":\"Prova\",\"email\":\"p@prova.local\",\"telefone\":\"0\",\"admin_usuario\":\"admin\",
 \"admin_senha\":\"senha-admin-longa\",\"senha_mestre\":\"senha-mestre-longa-da-prova\",
 \"servidor_nome\":\"vpn.prova.local\",\"servidor_ip\":\"192.168.89.1\",\"servidor_dns\":\"vpn.prova.local\",\"certificado_pem\":\"\"}" >/dev/null
TK_ADMIN=$(api POST /api/login "" '{"usuario":"admin","senha":"senha-admin-longa"}' | campo token)
api POST /api/redes "$TK_ADMIN" '{"nome":"Matriz","senha":"senha-da-rede","finalidade":"prova"}' >/dev/null
api POST /api/usuarios "$TK_ADMIN" '{"login":"ana","senha":"senha-da-ana-longa","email":"ana@prova.local"}' >/dev/null
TK_ANA=$(api POST /api/login "" '{"usuario":"ana","senha":"senha-da-ana-longa"}' | campo token)

# --- Cadastro do autenticador: o segredo sai UMA vez; o codigo e do Python.
SEGREDO=$(api POST /api/mfa/iniciar "$TK_ANA" '{}' | campo segredo)
api POST /api/mfa/confirmar "$TK_ANA" "{\"codigo\":\"$(codigo 0)\"}" >/dev/null
echo "== ana cadastrou o autenticador (segredo de ${#SEGREDO} caracteres base32)"
REDE_ID=$(api GET /api/redes "$TK_ADMIN" "" | python3 -c "import json,sys; print(json.load(sys.stdin)[0]['id'])")
api POST /api/redes/mfa "$TK_ADMIN" "{\"rede_id\":$REDE_ID,\"exige\":true}" | campo openvpn_reiniciado | sed 's/^/== openvpn reiniciado com o verificador: /'
api POST /api/redes/entrar "$TK_ANA" '{"nome":"Matriz","senha":"senha-da-rede"}' | campo perfil > "$T/ana.ovpn"
grep -q '^static-challenge "Código do autenticador" 1$' "$T/ana.ovpn" || { echo "FALHOU: perfil sem static-challenge"; exit 1; }
CONF="$T/dados/redes/$REDE_ID/servidor.conf"
grep -q "auth-user-pass-verify .*ovpn-mfa-verificar" "$CONF" || { echo "FALHOU: servidor sem verificador"; exit 1; }
GW=10.77.$(api GET /api/redes "$TK_ADMIN" "" | python3 -c "import json,sys; print(json.load(sys.stdin)[0]['subrede'].split('.')[2])").1
sleep 2

tentar() { # tentar ROTULO SENHA CODIGO -> linha JSON
  local r; r=$(C python3 "$AQUI/cliente.py" "$T/ana.ovpn" "$T/c-$1.log" ana "$2" "$3" "$GW")
  echo "== $1: $r" >&2; echo "$r"
}
res() { python3 -c "import json,sys; print(json.loads(sys.argv[1])['resultado'])" "$1"; }

VALIDO=$(codigo 30)   # passo seguinte ao do cadastro: dentro da janela de +1
R_SENHA=$(tentar senha-errada "senha-errada-da-ana" "$VALIDO")
R_CERTO=$(tentar certo "senha-da-ana-longa" "$VALIDO")
R_REUSO=$(tentar reuso "senha-da-ana-longa" "$VALIDO")
ERRADO=$( [ "$VALIDO" = "000000" ] && echo 111111 || echo 000000 )
R_ERRADO=$(tentar codigo-errado "senha-da-ana-longa" "$ERRADO")
R_SEM=$(tentar sem-codigo "senha-da-ana-longa" "")

# --- Revogacao (limites 6 e 12): conectado com o codigo, o admin muda o
# usuario -> o painel manda `client-kill` pela gerencia, a conexao cai em N s,
# e a reconexao pelo token e recusada (external-auth + credencial mudada).
# RED: com o soquete da gerencia fora do lugar, a mesma mudanca NAO derruba
# ninguem em 15 s -- a queda de cima vinha do painel, nao de outra coisa.
psql_prova() { # COMANDOS... no PostgreSQL da prova, como o painel o ve
  local a=(); for c in "$@"; do a+=(-c "$c"); done
  S env PGPASSWORD=senha-pg-prova "$PGBIN/psql" -h 127.0.0.1 -p $PGPORTA -U postgres -d postgres -q "${a[@]}"
}
membro_com_mfa() { # LOGIN SENHA -> segredo; cadastra o autenticador e entra na rede
  api POST /api/usuarios "$TK_ADMIN" "{\"login\":\"$1\",\"senha\":\"$2\"}" >/dev/null
  local tk seg
  tk=$(api POST /api/login "" "{\"usuario\":\"$1\",\"senha\":\"$2\"}" | campo token)
  seg=$(api POST /api/mfa/iniciar "$tk" '{}' | campo segredo)
  api POST /api/mfa/confirmar "$tk" "{\"codigo\":\"$(python3 "$AQUI/cliente.py" --codigo "$seg" 0)\"}" >/dev/null
  api POST /api/redes/entrar "$tk" '{"nome":"Matriz","senha":"senha-da-rede"}' | campo perfil > "$T/$1.ovpn"
  echo "$seg"
}
derrubada() { # ROTULO LOGIN SENHA SEGREDO ROTA CORPO SEGUNDOS -> JSON com n_segundos
  local saida="$T/manter-$1.json" pid t0
  : > "$saida"
  C env MANTER="$7" python3 "$AQUI/cliente.py" "$T/$2.ovpn" "$T/c-$1.log" "$2" "$3" \
    "$(python3 "$AQUI/cliente.py" --codigo "$4" 30)" > "$saida" &
  pid=$!
  for _ in $(seq 150); do grep -q '"evento": "conectou"' "$saida" && break; sleep 0.1; done
  sleep 3
  t0=$(date +%s.%N)
  case "$5" in
    sql) psql_prova "$6" >/dev/null ;;   # sem rota: so a vigia alcanca
    *) api POST "$5" "$TK_ADMIN" "$6" >/dev/null ;;
  esac
  wait "$pid" || true
  python3 - "$saida" "$t0" <<'PY'
import json, sys
linhas = [json.loads(l) for l in open(sys.argv[1]) if l.strip()]
r = linhas[-1]; t0 = float(sys.argv[2])
r["conectou_antes"] = any(l.get("evento") == "conectou" for l in linhas)
r["n_segundos"] = round(r["caiu_em"] - t0, 2) if "caiu_em" in r else None
r.pop("caiu_em", None)
print(json.dumps(r, ensure_ascii=False))
PY
}
SEG_CAIO=$(membro_com_mfa caio senha-do-caio-longa)
SEG_DANI=$(membro_com_mfa dani senha-da-dani-longa)
SEG_EDU=$(membro_com_mfa edu senha-do-edu-longa)
SEG_FABI=$(membro_com_mfa fabi senha-da-fabi-longa)
GER="$T/dados/gerencia/$REDE_ID.sock"
RV_ZERAR=$(derrubada zerar caio senha-do-caio-longa "$SEG_CAIO" /api/usuarios/mfa-zerar '{"login":"caio"}' 20)
echo "== admin zera o autenticador do caio conectado: $RV_ZERAR"
RV_DESATIVAR=$(derrubada desativar dani senha-da-dani-longa "$SEG_DANI" /api/usuarios/ativo '{"login":"dani","ativo":false}' 20)
echo "== admin desativa a dani conectada: $RV_DESATIVAR"
mv "$GER" "$GER.fora"
RV_RED=$(derrubada red-sem-gerencia edu senha-do-edu-longa "$SEG_EDU" /api/usuarios/ativo '{"login":"edu","ativo":false}' 15)
mv "$GER.fora" "$GER"
echo "== RED, gerencia fora do lugar, admin desativa o edu: $RV_RED"
# M2 da revisao SEC: UPDATE direto no banco, sem rota -- a vigia do painel
# (a cada credencial::VIGIA) tira o ccd e derruba. Mede N e o custo da
# pergunta que a vigia faz.
RV_SQL=$(derrubada sql-direto fabi senha-da-fabi-longa "$SEG_FABI" sql "UPDATE phx_usuario SET ativo = false WHERE login = 'fabi'" 20)
echo "== UPDATE direto no banco desativa a fabi conectada: $RV_SQL"
CUSTO_VIGIA=$(psql_prova '\timing on' "SELECT id, credencial FROM phx_usuario" | grep -o "Time: [0-9.]* ms" | tail -1)
echo "== custo da pergunta da vigia: $CUSTO_VIGIA"
TOKEN_RECUSADO=$(grep -c "token recusado: a conta mudou" "$T/painel.log" || true)
DERRUBADAS=$(grep -c "derrubada (credencial mudou)" "$T/painel.log" || true)

LOGSRV="$T/dados/redes/$REDE_ID/openvpn.log"
ADIADO=$(grep -c "deferred" "$LOGSRV" || true)
PROPRIO=$(grep -c "UID set to phxvpn-ovpn" "$LOGSRV" || true)
RECUSAS=$(grep -c "recusado --" "$T/painel.log" || true)
VAZOU=$( { grep -c -e "senha-da-ana-longa" -e "senha-errada-da-ana" -e "$SEGREDO" "$T/painel.log" "$LOGSRV" || true; } | awk -F: '{s+=$2} END {print s}')

# --- Adverso 1 (ALTO 1): 64 logins SIMULTANEOS com a senha certa e codigo
# errado. A tentativa se reserva antes do PBKDF2: so LIVRES+1 = 6 chegam a
# conferir (401); o resto e 429. Antes do conserto passavam as 64.
api POST /api/usuarios "$TK_ADMIN" '{"login":"bia","senha":"senha-da-bia-longa"}' >/dev/null
TK_BIA=$(api POST /api/login "" '{"usuario":"bia","senha":"senha-da-bia-longa"}' | campo token)
SEG_BIA=$(api POST /api/mfa/iniciar "$TK_BIA" '{}' | campo segredo)
api POST /api/mfa/confirmar "$TK_BIA" "{\"codigo\":\"$(python3 "$AQUI/cliente.py" --codigo "$SEG_BIA" 0)\"}" >/dev/null
CONCORRENTES=$(S python3 - <<'PY'
import json, threading, urllib.request, urllib.error, collections
c = collections.Counter(); trava = threading.Lock(); comeco = threading.Barrier(64)
def um():
    r = urllib.request.Request("http://127.0.0.1:8479/api/login", method="POST",
        data=b'{"usuario":"bia","senha":"senha-da-bia-longa","codigo":"000000"}',
        headers={"Content-Type": "application/json"})
    comeco.wait()
    try: s = urllib.request.urlopen(r, timeout=120).status
    except urllib.error.HTTPError as e: s = e.code
    except Exception as e: s = type(e).__name__
    with trava: c[str(s)] += 1
f = [threading.Thread(target=um) for _ in range(64)]
[x.start() for x in f]; [x.join() for x in f]
print(json.dumps(dict(c)))
PY
)
echo "== 64 logins simultaneos, codigo errado: $CONCORRENTES"

# --- Adverso 2 (ALTO 2): o soquete do verificador. nobody nem conecta
# (arquivo 0660 do grupo phxvpn-ovpn); nobody COM o grupo conecta e o
# SO_PEERCRED recusa; phxvpn-ovpn e atendido (e a senha errada, recusada).
SOCK="$T/dados/verificar.sock"
GID_OVPN=$(getent group phxvpn-ovpn | cut -d: -f3)
perguntar() { # perguntar ARGS_SETPRIV...
  setpriv "$@" python3 - "$SOCK" <<'PY'
import json, socket, sys
p = {"rede": "1", "cn": "ana.1.00000000", "usuario": "ana", "senha": "errada", "codigo": "000000", "ip": "192.0.2.9"}
try:
    s = socket.socket(socket.AF_UNIX); s.settimeout(5); s.connect(sys.argv[1])
    s.sendall((json.dumps(p) + "\n").encode()); r = s.makefile().readline().strip()
    print(r or "fechado")
except Exception as e:
    print(type(e).__name__)
PY
}
SOCK_NOBODY=$(perguntar --reuid=65534 --regid=65534 --clear-groups)
SOCK_NOBODY_GRUPO=$(perguntar --reuid=65534 --regid=65534 --groups="$GID_OVPN")
SOCK_OVPN=$(perguntar --reuid="$(id -u phxvpn-ovpn)" --regid="$GID_OVPN" --clear-groups)
echo "== soquete: nobody=$SOCK_NOBODY; nobody+grupo=$SOCK_NOBODY_GRUPO; phxvpn-ovpn=$SOCK_OVPN"
RECUSA_PEERCRED=$(grep -c "usuário não autorizado" "$T/painel.log" || true)

# --- RED: a conferencia trocada por /bin/true, mesmo servidor, mesmo perfil.
# pgrep veria o openvpn de outras provas: so o deste netns, pelo conf dele.
servidores() { for p in $(ip netns pids $NS_S); do
  tr '\0' ' ' < /proc/$p/cmdline 2>/dev/null | grep -q "openvpn --config $CONF" && echo "$p"; done; }
PIDS_OVPN=$(servidores); echo "== RED: parando o openvpn da rede (pid $PIDS_OVPN)"
for p in $PIDS_OVPN; do kill "$p" 2>/dev/null || true; done
for _ in $(seq 30); do [ -z "$(servidores)" ] && break; sleep 0.2; done
sed -i 's#^auth-user-pass-verify .*#auth-user-pass-verify /bin/true via-file#' "$CONF"
S openvpn --config "$CONF" --daemon --log "$T/red-servidor.log" --writepid "$T/red.pid"
sleep 2
RED_SENHA=$(tentar red-senha-errada "senha-errada-da-ana" "$ERRADO")
RED_CODIGO=$(tentar red-codigo-errado "senha-da-ana-longa" "$ERRADO")
kill "$(cat "$T/red.pid")" 2>/dev/null || true

OK=1
[ "$(res "$R_SENHA")" = recusado ] || { echo "FALHOU: senha errada nao foi recusada"; OK=0; }
[ "$(res "$R_CERTO")" = conectou ] || { echo "FALHOU: codigo certo nao conectou"; OK=0; }
[ "$(res "$R_REUSO")" = recusado ] || { echo "FALHOU: codigo reutilizado passou"; OK=0; }
[ "$(res "$R_ERRADO")" = recusado ] || { echo "FALHOU: codigo errado passou"; OK=0; }
[ "$(res "$R_SEM")" = recusado ] || { echo "FALHOU: sem codigo passou"; OK=0; }
[ "$PROPRIO" -ge 1 ] || { echo "FALHOU: o openvpn nao desceu para phxvpn-ovpn"; OK=0; }
[ "$VAZOU" = 0 ] || { echo "FALHOU: senha ou segredo em log"; OK=0; }
N401=$(python3 -c "import json,sys; print(json.loads(sys.argv[1]).get('401',0))" "$CONCORRENTES")
N429=$(python3 -c "import json,sys; print(json.loads(sys.argv[1]).get('429',0))" "$CONCORRENTES")
{ [ "$N401" -le 6 ] && [ $((N401 + N429)) = 64 ]; } || { echo "FALHOU: concorrencia furou o limitador ($CONCORRENTES)"; OK=0; }
[ "$SOCK_NOBODY" = PermissionError ] || { echo "FALHOU: nobody conectou no soquete"; OK=0; }
{ [ "$SOCK_NOBODY_GRUPO" = '{"ok":false}' ] && [ "$RECUSA_PEERCRED" -ge 1 ]; } || { echo "FALHOU: SO_PEERCRED nao recusou nobody"; OK=0; }
[ "$SOCK_OVPN" = '{"ok":false}' ] || { echo "FALHOU: phxvpn-ovpn nao foi atendido"; OK=0; }
# RED: sem a conferencia, as duas tentativas erradas conectam -- se nao
# conectassem, os "recusado" de cima podiam vir de outra coisa.
RED_OK=0
[ "$(res "$RED_SENHA")" = conectou ] && [ "$(res "$RED_CODIGO")" = conectou ] && RED_OK=1
[ $RED_OK = 1 ] || { echo "FALHOU: o RED nao conectou -- a recusa nao vinha da conferencia"; OK=0; }
caiu() { python3 -c "import json,sys; r=json.loads(sys.argv[1]); print(int(r['conectou_antes'] and r['n_segundos'] is not None and r['n_segundos'] <= 10 and not r.get('voltou')))" "$1"; }
[ "$(caiu "$RV_ZERAR")" = 1 ] || { echo "FALHOU: zerar o autenticador nao derrubou o caio em 10 s"; OK=0; }
[ "$(caiu "$RV_DESATIVAR")" = 1 ] || { echo "FALHOU: desativar nao derrubou a dani em 10 s"; OK=0; }
[ "$(caiu "$RV_SQL")" = 1 ] || { echo "FALHOU: UPDATE direto no banco nao derrubou a fabi em 10 s"; OK=0; }
python3 -c "import json,sys; r=json.loads(sys.argv[1]); sys.exit(0 if r['conectou_antes'] and r['n_segundos'] is None else 1)" "$RV_RED" \
  || { echo "FALHOU: o RED caiu sem a gerencia -- a queda nao vinha do painel"; OK=0; }
[ "$TOKEN_RECUSADO" -ge 3 ] || { echo "FALHOU: a reconexao pelo token nao foi recusada ($TOKEN_RECUSADO)"; OK=0; }

python3 - "$AQUI/resultados.json" <<PY
import json, sys, datetime
r = lambda s: json.loads(s)
json.dump({
  "prova": "phxvpn: usuario + senha + TOTP na conexao OpenVPN (auth-user-pass-verify via-file + static-challenge)",
  "data": datetime.datetime.now().astimezone().isoformat(timespec="seconds"),
  "openvpn": "$(openvpn --version | head -1 | cut -d' ' -f2)",
  "phxvpn_compilado_em": "$(basename "$(dirname "$BIN")")",
  "casos": {
    "senha_errada_com_codigo_valido": r('''$R_SENHA'''),
    "codigo_certo": r('''$R_CERTO'''),
    "mesmo_codigo_de_novo": r('''$R_REUSO'''),
    "codigo_errado": r('''$R_ERRADO'''),
    "sem_codigo": r('''$R_SEM'''),
  },
  "adverso_64_logins_simultaneos_codigo_errado": r('''$CONCORRENTES'''),
  "adverso_soquete": {"nobody": "$SOCK_NOBODY", "nobody_com_grupo": r('''$SOCK_NOBODY_GRUPO'''),
                      "recusas_por_peercred_no_log": $RECUSA_PEERCRED, "phxvpn_ovpn": r('''$SOCK_OVPN''')},
  "revogacao": {
    "zerar_autenticador_conectado": r('''$RV_ZERAR'''),
    "desativar_usuario_conectado": r('''$RV_DESATIVAR'''),
    "red_sem_gerencia_desativar": r('''$RV_RED'''),
    "update_direto_no_banco_desativar": r('''$RV_SQL'''),
    "custo_da_pergunta_da_vigia": "$CUSTO_VIGIA",
    "reconexoes_pelo_token_recusadas_no_log": $TOKEN_RECUSADO,
    "conexoes_derrubadas_no_log": $DERRUBADAS,
  },
  "red_conferencia_trocada_por_bin_true": {
    "senha_errada": r('''$RED_SENHA'''),
    "codigo_errado": r('''$RED_CODIGO'''),
  },
  "servidor": {"autenticacoes_adiadas": $ADIADO, "uid_phxvpn_ovpn": $PROPRIO >= 1,
               "recusas_no_log_do_painel": $RECUSAS,
               "senha_ou_segredo_em_log": $VAZOU},
  "passou": $OK == 1,
}, open(sys.argv[1], "w"), ensure_ascii=False, indent=2)
PY
echo "== adiadas no servidor: $ADIADO; recusas no painel: $RECUSAS; vazamentos em log: $VAZOU"
[ $OK = 1 ] && echo "== PROVA OK" || { echo "== PROVA REPROVADA"; exit 1; }
