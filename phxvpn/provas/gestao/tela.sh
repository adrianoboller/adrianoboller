#!/usr/bin/env bash
# Sobe um painel descartavel (PostgreSQL proprio, sem OpenVPN) e exercita no
# Chromium: trocar a propria senha, o admin desativar/reativar um usuario e
# remover um membro de rede -- os tres pedidos 456/457/458 (mesmo padrao de
# `provas/mfa/tela.sh`). Capturas em SAIDA.
# Uso: sudo ./tela.sh [pasta]
set -euo pipefail
AQUI=$(cd "$(dirname "$0")" && pwd)
RAIZ=$(cd "$AQUI/../.." && pwd)
T=${1:-$(mktemp -d /tmp/phxvpn-tela-gestao.XXXX)}
BIN=${PHXVPN_BIN:-$RAIZ/target/debug/phxvpn}
PGBIN=$(ls -d /usr/lib/postgresql/*/bin | sort -V | tail -1)
PGPORTA=55494; PAINEL=127.0.0.1:8482
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
api POST /api/instalar "" "{\"codigo_instalacao\":\"$CODIGO\",\"empresa\":\"Prova Ltda\",\"responsavel\":\"P\",\"email\":\"p@p.local\",
 \"admin_usuario\":\"admin\",\"admin_senha\":\"senha-admin-longa\",\"senha_mestre\":\"senha-mestre-longa-da-prova\",
 \"servidor_nome\":\"vpn1\",\"servidor_ip\":\"192.0.2.1\"}" >/dev/null
TK=$(api POST /api/login "" '{"usuario":"admin","senha":"senha-admin-longa"}' | python3 -c "import json,sys; print(json.load(sys.stdin)['token'])")
REDE=$(api POST /api/redes "$TK" '{"nome":"Matriz","senha":"senha-da-rede"}')
api POST /api/usuarios "$TK" '{"login":"ana","senha":"senha-da-ana-longa"}' >/dev/null
TK_ANA=$(api POST /api/login "" '{"usuario":"ana","senha":"senha-da-ana-longa"}' | python3 -c "import json,sys; print(json.load(sys.stdin)['token'])")
api POST /api/redes/entrar "$TK_ANA" '{"nome":"Matriz","senha":"senha-da-rede"}' >/dev/null
PAINEL="http://$PAINEL/" SAIDA=${SAIDA:-$T} /opt/node22/bin/node "$AQUI/tela.mjs"
