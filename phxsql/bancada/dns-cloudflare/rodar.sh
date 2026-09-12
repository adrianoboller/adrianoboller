#!/usr/bin/env bash
# Prova do provisionamento de DNS no Cloudflare, contra um Cloudflare FALSO
# local -- sem tocar infra real, sem gastar token. Nos DOIS sentidos:
# cria certo, e idempotente, atualiza no IP novo, e FALHA com token errado.
set -u
cd "$(dirname "$0")"

TOKEN="token-de-teste"
PORT="$(python3 -c 'import socket;s=socket.socket();s.bind(("127.0.0.1",0));print(s.getsockname()[1]);s.close()')"
BASE="http://127.0.0.1:${PORT}/client/v4"
ZONAS='{"phxmail.com.br":"zona-phxmail"}'

# a ferramenta e o falso nao passam pelo proxy do ambiente (e localhost)
run_tool(){ env -u HTTP_PROXY -u HTTPS_PROXY -u http_proxy -u https_proxy no_proxy=127.0.0.1 \
    CF_API_TOKEN="$1" CF_API_BASE="$BASE" CF_ZONAS="$ZONAS" \
    python3 provisionar_dns.py "$2" "$3"; }
estado(){ curl -s --noproxy 127.0.0.1 "http://127.0.0.1:${PORT}/__estado"; }

FAKE_CF_TOKEN="$TOKEN" python3 cloudflare_falso.py "$PORT" >/tmp/cf-falso.log 2>&1 &
FAKE=$!
trap 'kill $FAKE 2>/dev/null' EXIT

# espera subir
for _ in $(seq 1 50); do curl -s --noproxy 127.0.0.1 "http://127.0.0.1:${PORT}/__estado" >/dev/null 2>&1 && break; sleep 0.1; done

OK=0; FALHAS=0
passo(){ if [ "$1" = "ok" ]; then OK=$((OK+1)); printf '    ok    %s\n' "$2"; else FALHAS=$((FALHAS+1)); printf '   FALHA  %s\n' "$2"; fi; }
# confere via python lendo o /__estado (assercoes por MEDIDA, nao de memoria)
confere(){ estado | python3 -c "$1" "$2" "$3" && passo ok "$4" || passo x "$4"; }

echo "===== provisionamento de DNS no Cloudflare (falso) ====="

echo "1) primeira vez: cria o registro do IP fixo"
SAIDA1="$(run_tool "$TOKEN" prado 203.0.113.10)"; RC1=$?
[ $RC1 -eq 0 ] && passo ok "a ferramenta terminou com exito" || passo x "a ferramenta terminou com exito"
echo "$SAIDA1" | grep -q 'criado.*prado.phxmail.com.br' && passo ok "criou prado.phxmail.com.br" || passo x "criou prado.phxmail.com.br"
confere '
import json,sys
r=json.load(sys.stdin)["result"]
nomes=sorted(x["name"] for x in r)
assert nomes==["prado.phxmail.com.br"], nomes
assert all(x["content"]=="203.0.113.10" for x in r), r
assert all(x["type"]=="A" and x["proxied"] is False for x in r), r
' - - "no falso: 1 registro A, nome e IP certos, proxied=false"

echo "2) idempotente: rodar de novo NAO duplica -- atualiza"
SAIDA2="$(run_tool "$TOKEN" prado 203.0.113.10)"; RC2=$?
[ $RC2 -eq 0 ] && passo ok "segunda corrida com exito" || passo x "segunda corrida com exito"
echo "$SAIDA2" | grep -q 'atualizado.*phxmail' && passo ok "reconheceu o existente (atualizado, nao criado)" || passo x "reconheceu o existente"
confere '
import json,sys
r=json.load(sys.stdin)["result"]
assert len(r)==1, "duplicou! agora ha %d"%len(r)
' - - "no falso: continua com 1 registro (sem duplicata)"

echo "3) IP mudou: roda de novo e atualiza o conteudo dos dois"
run_tool "$TOKEN" prado 203.0.113.11 >/dev/null;
confere '
import json,sys
r=json.load(sys.stdin)["result"]
assert len(r)==1 and all(x["content"]=="203.0.113.11" for x in r), r
' - - "no falso: 1 registro, agora no IP novo 203.0.113.11"

echo "4) prova reversa: token ERRADO tem de FALHAR (a prova pega o erro)"
ANTES="$(estado)"
if run_tool "token-errado" prado 203.0.113.11 >/dev/null 2>&1; then passo x "token errado deveria falhar -- mas passou"; else passo ok "token errado FALHOU (403), como deve"; fi
[ "$(estado)" = "$ANTES" ] && passo ok "estado intacto apos a tentativa recusada" || passo x "estado mudou com token errado"

echo "5) segunda empresa nao mexe na primeira"
run_tool "$TOKEN" timeagil 198.51.100.20 >/dev/null
confere '
import json,sys
r=json.load(sys.stdin)["result"]
assert len(r)==2, len(r)
prado=[x for x in r if x["name"].startswith("prado.")]
assert all(x["content"]=="203.0.113.11" for x in prado), prado
tim=sorted(x["name"] for x in r if x["name"].startswith("timeagil."))
assert tim==["timeagil.phxmail.com.br"], tim
' - - "no falso: 2 registros; prado intacto; timeagil no phxmail"

echo
echo "===== RESULTADO ====="
echo "$((OK+FALHAS)) checagens, $OK ok, $FALHAS falha(s)."
DATA="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
if [ $FALHAS -eq 0 ]; then EST="PROVA VERDE"; TODOS=true; else EST="PROVA VERMELHA"; TODOS=false; fi
echo "$EST"
cat > resultados.json <<JSON
{
  "bancada": "dns-cloudflare",
  "medido_em": "$DATA",
  "checagens": $((OK+FALHAS)),
  "ok": $OK,
  "falhas": $FALHAS,
  "todos_verdes": $TODOS,
  "observacao": "Cloudflare falso local; nenhum toque na API real; nenhum token real."
}
JSON
[ $FALHAS -eq 0 ]
