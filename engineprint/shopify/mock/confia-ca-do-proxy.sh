#!/usr/bin/env bash
# Poe no repositorio de certificados do Chromium (~/.pki/nssdb) os CAs da
# Anthropic que ja estao no pacote oficial do proxy (/root/.ccr/ca-bundle.crt).
#
# Sem isto o Chromium do Playwright recusa todo site com
# ERR_CERT_AUTHORITY_INVALID: o proxy intercepta a conexao do navegador, e o
# nssdb deste conteiner estava VAZIO (medido em 28/09/2026: zero
# certificados), embora o README do proxy diga que ele vem configurado.
# Nao desliga verificacao nenhuma: o navegador passa a confiar no mesmo CA em
# que curl, Node e Python ja confiam. Idempotente — pode rodar de novo.
#
# Uso:  bash mock/confia-ca-do-proxy.sh
set -euo pipefail

PACOTE=/root/.ccr/ca-bundle.crt
BASE="sql:${HOME}/.pki/nssdb"
[ -r "$PACOTE" ] || { echo "sem $PACOTE — este ambiente nao tem o proxy da Anthropic"; exit 2; }
command -v certutil >/dev/null || apt-get install -y -q libnss3-tools >/dev/null
mkdir -p "${HOME}/.pki/nssdb"
[ -f "${HOME}/.pki/nssdb/cert9.db" ] || certutil -N -d "$BASE" --empty-password

TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT
python3 - "$PACOTE" "$TMP" <<'EOF'
import re, subprocess, sys
pacote, tmp = sys.argv[1], sys.argv[2]
certs = re.findall(r'-----BEGIN CERTIFICATE-----.*?-----END CERTIFICATE-----', open(pacote).read(), re.S)
vistos = set()
for c in certs:
    s = subprocess.run(['openssl', 'x509', '-noout', '-subject', '-fingerprint', '-sha256'],
                       input=c, capture_output=True, text=True).stdout
    if 'O = Anthropic' not in s:
        continue
    fp = s.strip().split('=')[-1].replace(':', '')[:16]
    if fp in vistos:
        continue
    vistos.add(fp)
    open(f'{tmp}/anthropic-{fp}.pem', 'w').write(c + '\n')
EOF

n=0
for pem in "$TMP"/*.pem; do
  nome=$(basename "$pem" .pem)
  certutil -A -d "$BASE" -t "C,," -n "$nome" -i "$pem"
  n=$((n + 1))
done
echo "$n CA(s) da Anthropic no nssdb:"
certutil -L -d "$BASE" | grep -c anthropic- || true
