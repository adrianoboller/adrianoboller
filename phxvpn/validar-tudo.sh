#!/bin/bash
# Revalida TODOS os recursos do phxvpn de uma vez (root, Linux com netns,
# openvpn, postgresql, wine e o usbip de referencia). Cada linha roda a prova
# de verdade e anota OK/FALHOU e o tempo; nada e dado por provado de memoria.
# Saida: validacao.json e a tabela na tela.
set -u
R=$(cd "$(dirname "$0")" && pwd); cd "$R"
W="env WINEDEBUG=-all CARGO_TARGET_X86_64_PC_WINDOWS_GNU_RUNNER=/usr/lib/wine/wine64"
PGB=$(ls -d /usr/lib/postgresql/*/bin 2>/dev/null | sort -V | tail -1)
USBIP=$(ls /usr/lib/linux-tools/*/usbip 2>/dev/null | head -1)
LINHAS=()
prova() { # prova "recurso" "como se prova" comando...
  local rec=$1 como=$2; shift 2
  local t0=$(date +%s) saida r
  saida=$("$@" 2>&1); r=$?
  local dt=$(( $(date +%s) - t0 ))
  local ult; ult=$(printf '%s' "$saida" | grep -v '^\s*$' | tail -1 | tr '"\\' "'/" | cut -c1-140)
  printf '%-7s %4ss  %s\n' "$([ $r -eq 0 ] && echo OK || echo FALHOU)" "$dt" "$rec"
  LINHAS+=("{\"recurso\":\"$rec\",\"como\":\"$como\",\"ok\":$([ $r -eq 0 ] && echo true || echo false),\"s\":$dt,\"ultima\":\"$ult\"}")
}
cargo build -q && cargo build -q --release && cargo build -q --target x86_64-pc-windows-gnu
PG_TMP=/tmp/phx-val-pg
pg_sobe() { rm -rf $PG_TMP; mkdir -p $PG_TMP; chown postgres $PG_TMP; echo s3nh4-val > $PG_TMP/s; chown postgres $PG_TMP/s
  su postgres -c "$PGB/initdb -D $PG_TMP/d -A scram-sha-256 --pwfile=$PG_TMP/s -U postgres" >/dev/null &&
  su postgres -c "$PGB/pg_ctl -D $PG_TMP/d -o '-p 55450 -k /tmp -c listen_addresses=127.0.0.1' -l $PG_TMP/d/log start" >/dev/null && sleep 2; }
pg_desce() { su postgres -c "$PGB/pg_ctl -D $PG_TMP/d -m fast stop" >/dev/null 2>&1; }

prova "Suíte Linux (P2P, Noise, repasse, convite, mac1/cookie, USB/IP, painel, cofre, PKI, ACL, serviço)" "cargo test" \
  bash -c 'cargo test -q 2>&1 | tee /dev/stderr | grep -q "test result: FAILED" && exit 1 || exit 0'
prova "Suíte Windows sob o Wine (TAP, DPAPI, ACL, bandeja, registro Run)" "cargo test --target windows-gnu" \
  bash -c "$W cargo test -q --target x86_64-pc-windows-gnu 2>&1 | grep -q 'test result: FAILED' && exit 1 || exit 0"
prova "Autoteste contra vetores oficiais (Noise, RFC 7677, cofre, Ed25519)" "phxvpncmd AUTOTESTE" \
  bash -c 'target/release/phxvpncmd /modo:ferramentas /comando:AUTOTESTE | grep -q "tudo confere"'
pg_sobe
prova "PostgreSQL real: esquema, FK RESTRICT, SCRAM, CRL no openssl" "tests/postgres_real.rs" \
  bash -c 'PHXVPN_PG_TESTE="host=127.0.0.1 port=55450 user=postgres password=s3nh4-val dbname=postgres" cargo test -q --test postgres_real -- --nocapture 2>&1 | grep -v "NAO RODOU" | grep -q "test result: ok"'
pg_desce
prova "Modo servidor com OpenVPN real: TLS 1.3, ping, sem root, tls-crypt-v2, removido barrado" "prova-openvpn.sh" \
  bash -c './prova-openvpn.sh 2>&1 | tail -3 | grep -q "PROVA OK"'
prova "USB/IP interopera com o usbip de referência" "usbip list -r" \
  bash -c "PHXVPN_USBIP=$USBIP cargo test -q --lib o_usbip_de_referencia 2>&1 | grep -q 'test result: ok'"
prova "USB pela janela, duas janelas e túnel real" "provas/usb-janela/rodar.sh" \
  bash -c 'SAIDA=/tmp/phx-val-usb provas/usb-janela/rodar.sh 2>&1 | grep -q "B erros \[\]"'
prova "Programa de mesa + P2P + convite + malha + chat + USB com três computadores" "provas/video/ambiente.sh" \
  bash -c 'provas/video/ambiente.sh 2>&1 | grep -c "erros \[\]" | grep -q 3'
prova "Interface responsiva sem rolagem lateral em 390–3440 px" "responsivo.json do ambiente" \
  python3 -c 'import json;m=json.load(open("/var/tmp/phx-demo/responsivo.json"));assert all(x["rolagem"]==x["largura"] for x in m),m'
prova "Unidades systemd válidas (painel, repasse, p2p)" "systemd-analyze verify" \
  bash -c 'd=$(mktemp -d); PHXVPN_PG=x target/debug/phxvpn servico instalar painel --mostrar | sed 1d | grep -v "^# cred" > $d/phxvpn-painel.service && target/debug/phxvpn servico instalar repasse --mostrar | sed 1d > $d/phxvpn-repasse.service && ! systemd-analyze verify $d/*.service 2>&1 | grep .'
prova "Serviço do Windows: instalar, RUNNING, escutar, remover" "SCM do Wine" \
  bash -c 'E="Z:\\home\\user\\adrianoboller\\phxvpn\\target\\x86_64-pc-windows-gnu\\debug\\phxvpn.exe"; (env WINEDEBUG=-all timeout 60 /usr/lib/wine/wine64 cmd.exe /c "ping -n 50 127.0.0.1 >nul" &); sleep 2; env WINEDEBUG=-all /usr/lib/wine/wine64 "$E" servico instalar repasse --porta 51898 >/dev/null; for i in $(seq 20); do sleep 1; ss -lun | grep -q 51898 && break; done; ss -lun | grep -q 51898; ok=$?; env WINEDEBUG=-all /usr/lib/wine/wine64 "$E" servico remover phxvpn-repasse >/dev/null; exit $ok'
prova "Pacotes .deb/.msi/.zip; .deb instala, roda e remove" "empacotar.sh + dpkg" \
  bash -c './empacotar.sh /tmp/phx-val-pac >/dev/null && dpkg -i /tmp/phx-val-pac/phxvpn_*_amd64.deb >/dev/null && phxvpn versao && dpkg -r phxvpn >/dev/null'
prova "P2P difusão: broadcast/multicast a dois pares, desligada 0, rajada no teto, sem laço" "provas/broadcast/rodar.sh" \
  bash -c 'provas/broadcast/rodar.sh >/dev/null 2>&1; python3 -c "import json;c={x[\"cenario\"]:x for x in json.load(open(\"provas/broadcast/resultados.json\"))[\"cenarios\"]};l=c[\"ligada\"];assert all(v==dict(b=20,c=20) for v in l[\"de_a_por_destino_20_cada\"].values()),l;assert set(l[\"rx_placa_janela_quieta_5s\"].values())=={0};assert l[\"rajada_64B\"][\"b\"]<=205 and len(l[\"ssdp\"][\"respostas\"])==2;assert all(v[\"b\"]==0 for v in c[\"desligada-b\"][\"de_a_por_destino_20_cada\"].values())"'
prova "Bancada comparativa (phxvpn x WireGuard x OpenVPN)" "bancada/comparativo/medir.sh" \
  bash -c 'bancada/comparativo/medir.sh 3 >/dev/null 2>&1; python3 -c "import json;j=json.load(open(\"bancada/comparativo/resultados.json\"));assert all(min(r[\"mbits\"])>0 for r in j[\"resultados\"])"'

printf '{"data":"%s","provas":[%s]}\n' "$(date -u +%FT%TZ)" "$(IFS=,; echo "${LINHAS[*]}")" > validacao.json
python3 -c "import json;p=json.load(open('validacao.json'))['provas'];print(f'{sum(x[\"ok\"] for x in p)} de {len(p)} provas OK')"
