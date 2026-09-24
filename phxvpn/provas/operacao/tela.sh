#!/usr/bin/env bash
# Sobe um painel descartavel (PostgreSQL proprio, sem OpenVPN), poe tres
# conexoes no historico pelo MESMO soquete que o gancho do openvpn usa, e
# exercita a tela no Chromium: o admin ve as tres e muda a retencao; a ana ve
# so a dela. Capturas em SAIDA (padrao: docs/previa).
# Uso: sudo ./tela.sh
set -euo pipefail
AQUI=$(cd "$(dirname "$0")" && pwd)
RAIZ=$(cd "$AQUI/../.." && pwd)
T=$(mktemp -d /tmp/phxvpn-tela-historico.XXXX)
BIN=${PHXVPN_BIN:-$RAIZ/target/debug/phxvpn}
PGBIN=$(ls -d /usr/lib/postgresql/*/bin | sort -V | tail -1)
PGPORTA=55464; PAINEL=127.0.0.1:8495
mkdir -p "$T/pg" "$T/pgsock"; chmod 755 "$T"; chown postgres "$T/pg" "$T/pgsock"
limpar() { set +e; [ -n "${PID:-}" ] && kill "$PID"; su postgres -c "$PGBIN/pg_ctl -D $T/pg -m fast stop" >/dev/null 2>&1; }
trap limpar EXIT
echo "senha-pg-tela" > "$T/pgsenha"; chown postgres "$T/pgsenha"
su postgres -c "$PGBIN/initdb -D $T/pg -A scram-sha-256 --pwfile=$T/pgsenha -U postgres" >/dev/null
su postgres -c "$PGBIN/pg_ctl -D $T/pg -o '-p $PGPORTA -k $T/pgsock -c listen_addresses=127.0.0.1' -l $T/pg/pg.log start" >/dev/null
sleep 2
PHXVPN_PG="host=127.0.0.1 port=$PGPORTA user=postgres password=senha-pg-tela dbname=postgres" \
  "$BIN" painel --dados "$T/dados" --escutar $PAINEL >"$T/painel.log" 2>&1 &
PID=$!
for _ in $(seq 50); do grep -q "CODIGO DE INSTALACAO" "$T/painel.log" && break; sleep 0.2; done
CODIGO=$(grep -o "CODIGO DE INSTALACAO: [^ ]*" "$T/painel.log" | awk '{print $4}')
api() { curl -sf -X "$1" "http://$PAINEL$2" -H 'Content-Type: application/json' ${3:+-H "Authorization: Bearer $3"} ${4:+-d "$4"}; }
token() { api POST /api/login "" "{\"usuario\":\"$1\",\"senha\":\"$2\"}" | python3 -c "import json,sys; print(json.load(sys.stdin)['token'])"; }
api POST /api/instalar "" "{\"codigo_instalacao\":\"$CODIGO\",\"empresa\":\"Prova Ltda\",\"responsavel\":\"P\",\"email\":\"p@p.local\",
 \"admin_usuario\":\"admin\",\"admin_senha\":\"senha-admin-longa\",\"senha_mestre\":\"senha-mestre-longa-da-prova\",
 \"servidor_nome\":\"vpn1\",\"servidor_ip\":\"192.0.2.1\"}" >/dev/null
TK=$(token admin senha-admin-longa)
api POST /api/redes "$TK" '{"nome":"Matriz","senha":"senha-da-rede"}' >/dev/null
for u in ana beto; do
  api POST /api/usuarios "$TK" "{\"login\":\"$u\",\"senha\":\"senha-da-$u-longa\"}" >/dev/null
  api POST /api/redes/entrar "$(token $u senha-da-$u-longa)" '{"nome":"Matriz","senha":"senha-da-rede"}' >/dev/null
done
# Tres conexoes pelo soquete do verificador, como o gancho do openvpn manda.
python3 - "$T/dados" <<'PY'
import json, os, socket, sys, time
dados = sys.argv[1]
cn = {f.split(".")[0]: f for f in os.listdir(os.path.join(dados, "redes/1/ccd"))}
agora = int(time.time())
def mandar(**c):
    s = socket.socket(socket.AF_UNIX); s.connect(os.path.join(dados, "verificar.sock"))
    s.sendall((json.dumps({k: str(v) for k, v in c.items()}) + "\n").encode())
    assert json.loads(s.makefile().readline())["ok"], c
    s.close()
mandar(tipo="saiu", rede=1, cn=cn["ana"], ip="198.51.100.7", porta=40001, ip_vpn="10.77.1.3",
       desde=agora - 7300, duracao=3725, bytes_do_membro=48_213_775, bytes_ao_membro=731_442_117)
mandar(tipo="entrou", rede=1, cn=cn["beto"], ip="203.0.113.44", porta=51230, ip_vpn="10.77.1.4", desde=agora - 900)
mandar(tipo="saiu", rede=1, cn=cn["beto"], ip="203.0.113.44", porta=50001, ip_vpn="10.77.1.4",
       desde=agora - 40 * 86400, duracao=42, bytes_do_membro=18_220, bytes_ao_membro=9_931)
PY
PAINEL="http://$PAINEL/" SAIDA=${SAIDA:-$RAIZ/docs/previa} /opt/node22/bin/node "$AQUI/tela.mjs"
