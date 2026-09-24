#!/usr/bin/env bash
# Prova do CICLO DE VIDA do OpenVPN no modo servidor, com o openvpn 2.6 de
# verdade -- itens 3, 5 e 7 do documento de lacunas (24/09/2026):
#
#   saida    -- membro sai (SIGTERM no openvpn dele): em quanto tempo some do
#               status.log do servidor (a lista de membros do painel)?
#   reinicio -- o painel reinicia o OpenVPN da rede (mudar a exigencia do
#               autenticador): em quanto tempo os dois membros religam?
#   log      -- com teto pequeno (PHXVPN_OVPN_LOG_TETO), o openvpn.log gira?
#               Alguma linha partida? O openvpn segura algum arquivo?
#
# Roda o MESMO roteiro com dois binarios: o novo (VERDE) e o de antes
# (VELHO, o RED): sem `explicit-exit-notify`, com SIGKILL e com o log em
# append pelo proprio openvpn. Os dois sentidos saem no mesmo resultados.json.
#
# Topologia: tres netns proprios -- servidor (PostgreSQL, painel, openvpn)
# com uma ponte, e um netns por membro. Nenhuma porta do hospedeiro;
# processos morrem por PID.
#
# Uso: sudo PHXVPN_BIN_VELHO=/caminho/phxvpn-de-antes ./rodar.sh
set -euo pipefail

AQUI=$(cd "$(dirname "$0")" && pwd)
RAIZ=$(cd "$AQUI/../.." && pwd)
BIN_NOVO=${PHXVPN_BIN:-$RAIZ/target/debug/phxvpn}
BIN_VELHO=${PHXVPN_BIN_VELHO:-}
PGBIN=$(ls -d /usr/lib/postgresql/*/bin | sort -V | tail -1)
PGPORTA=55497
PAINEL=127.0.0.1:8483
TETO=4096
NS_S=pxcic-s; NS_A=pxcic-a; NS_B=pxcic-b

limpar() {
  set +e
  for n in $NS_A $NS_B $NS_S; do ip netns pids $n 2>/dev/null | xargs -r kill; done
  sleep 1
  [ -n "${T:-}" ] && [ -d "$T/pg" ] && ip netns exec $NS_S su postgres -c "$PGBIN/pg_ctl -D $T/pg -m fast stop" >/dev/null 2>&1
  for n in $NS_A $NS_B $NS_S; do ip netns pids $n 2>/dev/null | xargs -r kill -9; ip netns del $n 2>/dev/null; rm -rf /etc/netns/$n; done
}
S() { ip netns exec $NS_S "$@"; }

rodada() { # rodada ROTULO BINARIO -> uma linha JSON
  local rotulo=$1 bin=$2
  T=$(mktemp -d /tmp/phxvpn-prova-ciclo.XXXX); chmod 755 "$T"
  echo "== [$rotulo] pasta: $T ($bin)" >&2
  limpar
  set -e
  ip netns add $NS_S; ip netns add $NS_A; ip netns add $NS_B
  S ip link set lo up
  S ip link add pxcbr type bridge; S ip addr add 192.168.91.1/24 dev pxcbr; S ip link set pxcbr up
  local i=1
  for n in $NS_A $NS_B; do
    ip -n $n link set lo up
    ip link add pxcv$i netns $NS_S type veth peer name eth0 netns $n
    S ip link set pxcv$i master pxcbr up
    ip -n $n addr add 192.168.91.1$i/24 dev eth0; ip -n $n link set eth0 up
    mkdir -p /etc/netns/$n; echo "192.168.91.1 vpn.prova.local" > /etc/netns/$n/hosts
    i=$((i + 1))
  done

  mkdir -p "$T/pg" "$T/pgsock"; chown postgres "$T/pg" "$T/pgsock"
  echo "senha-pg-prova" > "$T/pgsenha"; chown postgres "$T/pgsenha"
  su postgres -c "$PGBIN/initdb -D $T/pg -A scram-sha-256 --pwfile=$T/pgsenha -U postgres" >/dev/null
  S su postgres -c "$PGBIN/pg_ctl -D $T/pg -o '-p $PGPORTA -k $T/pgsock -c listen_addresses=127.0.0.1' -l $T/pg/pg.log start" >/dev/null
  sleep 2

  # Sem funcao no meio: `ip netns exec` e `env` fazem exec, entao `$!` e o
  # pid do painel. O openvpn do servidor e FILHO dele -- procurar por nome
  # pegaria o de outra prova na mesma maquina.
  ip netns exec $NS_S env PHXVPN_OVPN_LOG_TETO=$TETO \
    PHXVPN_PG="host=127.0.0.1 port=$PGPORTA user=postgres password=senha-pg-prova dbname=postgres" \
    "$bin" painel --dados "$T/dados" --openvpn --escutar $PAINEL >"$T/painel.log" 2>&1 &
  local pid_painel=$!
  for _ in $(seq 50); do grep -q "CODIGO DE INSTALACAO" "$T/painel.log" && break; sleep 0.2; done
  local codigo; codigo=$(grep -o "CODIGO DE INSTALACAO: [^ ]*" "$T/painel.log" | awk '{print $4}')

  api() {
    S python3 - "$@" <<'PY'
import json, sys, urllib.request
m, c, tk, corpo = sys.argv[1:5]
r = urllib.request.Request("http://127.0.0.1:8483" + c, method=m,
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
 \"servidor_nome\":\"vpn.prova.local\",\"servidor_ip\":\"192.168.91.1\",\"servidor_dns\":\"vpn.prova.local\",\"certificado_pem\":\"\"}" >/dev/null
  local tk_admin tk_ana rede_id
  tk_admin=$(api POST /api/login "" '{"usuario":"admin","senha":"senha-admin-longa"}' | campo token)
  api POST /api/redes "$tk_admin" '{"nome":"Matriz","senha":"senha-da-rede","finalidade":"prova"}' | campo perfil > "$T/admin.ovpn"
  api POST /api/usuarios "$tk_admin" '{"login":"ana","senha":"senha-da-ana-longa","email":"ana@prova.local"}' >/dev/null
  tk_ana=$(api POST /api/login "" '{"usuario":"ana","senha":"senha-da-ana-longa"}' | campo token)
  api POST /api/redes/entrar "$tk_ana" '{"nome":"Matriz","senha":"senha-da-rede"}' | campo perfil > "$T/ana.ovpn"
  rede_id=$(api GET /api/redes "$tk_admin" "" | python3 -c "import json,sys; print(json.load(sys.stdin)[0]['id'])")
  local dir="$T/dados/redes/$rede_id" status="$T/dados/redes/$rede_id/status.log"
  sleep 1

  sobe() { # NETNS PERFIL LOG
    ip netns exec "$1" openvpn --config "$2" --log "$3" --daemon --writepid "$3.pid"
  }
  completos() { grep -c "Initialization Sequence Completed" "$1" 2>/dev/null || true; }
  na_lista() { grep -c "^CLIENT_LIST," "$status" 2>/dev/null || true; }
  esperar() { # SEGUNDOS COMANDO... -> segundos ate o comando dar certo, ou null
    local fim t0; t0=$(date +%s.%N); fim=$(( $(date +%s) + $1 )); shift
    while [ "$(date +%s)" -lt "$fim" ]; do
      if "$@"; then python3 -c "import sys,time; print(round(time.time()-float(sys.argv[1]),1))" "$t0"; return; fi
      sleep 0.2
    done
    echo null
  }
  lista_tem() { [ "$(na_lista)" -eq "$1" ]; }
  religou() { [ "$(completos "$1")" -gt "$2" ]; }

  sobe $NS_A "$T/admin.ovpn" "$T/a.log"; sobe $NS_B "$T/ana.ovpn" "$T/b.log"
  local t_lista; t_lista=$(esperar 60 lista_tem 2)
  echo "== [$rotulo] dois membros na lista em ${t_lista} s" >&2
  local perfil_een=0 conf_een=0
  grep -q "^explicit-exit-notify 1$" "$T/ana.ovpn" && perfil_een=1
  grep -q "^explicit-exit-notify 1$" "$dir/servidor.conf" && conf_een=1

  # --- saida: a ana sai com SIGTERM (o que o fechar do cliente faz)
  kill -TERM "$(cat "$T/b.log.pid")"
  local t_saida; t_saida=$(esperar 200 lista_tem 1)
  echo "== [$rotulo] ana saiu da lista em ${t_saida} s (perfil com explicit-exit-notify: $perfil_een)" >&2

  # --- reinicio: a ana volta; o admin muda a exigencia (reinicia o openvpn)
  sleep 1; sobe $NS_B "$T/ana.ovpn" "$T/b2.log"
  esperar 60 lista_tem 2 >/dev/null
  sleep 3
  local ca cb pid_velho; ca=$(completos "$T/a.log"); cb=$(completos "$T/b2.log")
  pid_velho=$(pgrep -P "$pid_painel" -x openvpn || true)
  local t0; t0=$(date +%s.%N)
  api POST /api/redes/mfa "$tk_admin" "{\"rede_id\":$rede_id,\"exige\":false}" >/dev/null
  local t_api; t_api=$(python3 -c "import sys,time; print(round(time.time()-float(sys.argv[1]),1))" "$t0")
  local t_a t_b pid_novo
  # Os dois contados do MESMO instante (o clique): esperar um depois do outro
  # mediria o segundo a partir da volta do primeiro -- erro da 1a corrida.
  read -r t_a t_b < <(python3 - "$t0" "$T/a.log" "$ca" "$T/b2.log" "$cb" <<'PY'
import sys, time
t0 = float(sys.argv[1]); alvos = [(sys.argv[2], int(sys.argv[3])), (sys.argv[4], int(sys.argv[5]))]
res = [None, None]
while time.time() - t0 < 150 and None in res:
    for i, (log, antes) in enumerate(alvos):
        if res[i] is None and open(log, errors="replace").read().count("Initialization Sequence Completed") > antes:
            res[i] = round(time.time() - t0, 1)
    time.sleep(0.2)
print(*["null" if r is None else r for r in res])
PY
)
  pid_novo=$(pgrep -P "$pid_painel" -x openvpn || true)
  [ -n "$pid_velho" ] && [ "$pid_novo" != "$pid_velho" ] || { echo "FALHOU: o openvpn do servidor nao foi reiniciado" >&2; exit 1; }
  local restart_no_cliente=0
  grep -q "server-pushed-connection-reset\|Server poke\|RESTART" "$T/a.log" && restart_no_cliente=1
  echo "== [$rotulo] reinicio: a chamada levou ${t_api} s; admin religou em ${t_a} s, ana em ${t_b} s, contados do clique (RESTART recebido: $restart_no_cliente)" >&2

  # --- log: arquivos, tamanhos, linhas partidas, quem segura o que
  local pid_ovpn fd1 arquivos maior partidas deletados
  pid_ovpn=$(pgrep -P "$pid_painel" -x openvpn || true)
  fd1=$(readlink "/proc/$pid_ovpn/fd/1" || echo "?")
  arquivos=$(ls "$dir"/openvpn.log* | wc -l)
  maior=$(stat -c %s "$dir"/openvpn.log* | sort -n | tail -1)
  partidas=$(cat "$dir"/openvpn.log* | grep -cvE '^[0-9]{4}-[0-9]{2}-[0-9]{2} [0-9]{2}:[0-9]{2}:[0-9]{2} ' || true)
  deletados=$(ls -l /proc/"$pid_painel"/fd /proc/"$pid_ovpn"/fd 2>/dev/null | grep -c "(deleted)" || true)
  echo "== [$rotulo] log: $arquivos arquivo(s), maior $maior B (teto $TETO), linhas sem carimbo $partidas, fd1 do openvpn -> $fd1, descritores de apagado $deletados" >&2

  python3 - "$rotulo" "$bin" "$perfil_een" "$conf_een" "$t_lista" "$t_saida" "$t_api" "$t_a" "$t_b" \
    "$restart_no_cliente" "$arquivos" "$maior" "$partidas" "$fd1" "$deletados" "$TETO" <<'PY'
import json, sys
a = sys.argv[1:]
num = lambda s: None if s == "null" else float(s)
print(json.dumps({
    "rodada": a[0], "binario": a[1],
    "perfil_com_explicit_exit_notify": a[2] == "1",
    "servidor_com_explicit_exit_notify": a[3] == "1",
    "s_dois_na_lista": num(a[4]),
    "s_membro_sai_da_lista": num(a[5]),
    "s_api_reiniciar": num(a[6]),
    "s_admin_religou_desde_o_clique": num(a[7]),
    "s_ana_religou_desde_o_clique": num(a[8]),
    "cliente_recebeu_restart": a[9] == "1",
    "log_arquivos": int(a[10]), "log_maior_bytes": int(a[11]), "log_teto_bytes": int(a[15]),
    "log_linhas_sem_carimbo": int(a[12]),
    "openvpn_fd1": a[13],
    "descritores_de_arquivo_apagado": int(a[14]),
}, ensure_ascii=False))
PY
  limpar
}

trap limpar EXIT
VERDE=$(rodada novo "$BIN_NOVO")
VELHO=null
[ -n "$BIN_VELHO" ] && VELHO=$(rodada velho "$BIN_VELHO")
python3 - "$VERDE" "$VELHO" "$AQUI/resultados.json" <<'PY'
import json, sys, datetime, platform
novo, velho = json.loads(sys.argv[1]), json.loads(sys.argv[2])
ok = (novo["perfil_com_explicit_exit_notify"] and novo["servidor_com_explicit_exit_notify"]
      and novo["s_membro_sai_da_lista"] is not None and novo["s_membro_sai_da_lista"] < 20
      and all(novo[k] is not None and novo[k] < 20 for k in ("s_admin_religou_desde_o_clique", "s_ana_religou_desde_o_clique"))
      and novo["log_arquivos"] >= 2 and novo["log_maior_bytes"] <= novo["log_teto_bytes"]
      and novo["log_linhas_sem_carimbo"] == 0 and novo["openvpn_fd1"].startswith("pipe:")
      and novo["descritores_de_arquivo_apagado"] == 0)
# O RED so vale se o velho CONECTOU: sem isso ele reprova por nao entrar,
# nao pelo que a prova mede (ver a cognicao do RED que falha pelo motivo
# errado -- binario velho numa pasta que o `nobody` nao le).
red = velho is None or (
      velho["s_dois_na_lista"] is not None
      and (velho["s_membro_sai_da_lista"] is None or velho["s_membro_sai_da_lista"] > 60)
      and all(velho[k] is None or velho[k] > 30 for k in ("s_admin_religou_desde_o_clique", "s_ana_religou_desde_o_clique"))
      and velho["log_arquivos"] == 1 and not velho["openvpn_fd1"].startswith("pipe:"))
r = {
  "prova": "ciclo do OpenVPN no modo servidor: saida do membro, reinicio com aviso, log com teto",
  "medido_em": datetime.datetime.now(datetime.timezone.utc).isoformat(timespec="seconds"),
  "kernel": platform.release(),
  "n": 1,
  "novo": novo, "velho_red": velho,
  "confere_verde": ok, "confere_red": red if velho is not None else None,
}
open(sys.argv[3], "w").write(json.dumps(r, ensure_ascii=False, indent=1) + "\n")
print(json.dumps(r, ensure_ascii=False, indent=1))
sys.exit(0 if ok and (velho is None or red) else 1)
PY
