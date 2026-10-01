#!/usr/bin/env bash
# Kit dos portoes que so o dono fecha, na maquina Windows dele:
#   - dispositivos: phxclaw.exe (servidor WSS) + phxclaw-device-node.exe, com CA de TESTE
#   - U4b: o WLanguage gerado do mesmo SQL que o Rust compila e testa, para o WinDev
# Uso: tools/kit_do_dono.sh <pasta-de-saida>     (gera <pasta>/ e <pasta>.zip)
# Exige: alvo x86_64-pc-windows-gnu, mingw-w64, openssl, zip.
# A CA vai junto e vale 30 dias: e material de TESTE para localhost, nunca de producao.
set -euo pipefail
SAIDA=$(realpath -m "${1:?uso: kit_do_dono.sh <pasta-de-saida>}")
RAIZ=$(cd "$(dirname "$0")/.." && pwd)
ALVO=x86_64-pc-windows-gnu
rm -rf "$SAIDA" && mkdir -p "$SAIDA/dispositivos" "$SAIDA/u4b-wlanguage"

export CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER=x86_64-w64-mingw32-gcc
(cd "$RAIZ" && cargo build --release -q -p phxclaw -p phxclaw-device-node --target $ALVO)
cp "$RAIZ/target/$ALVO/release/phxclaw.exe" "$RAIZ/target/$ALVO/release/phxclaw-device-node.exe" \
   "$SAIDA/dispositivos/"

# CA de teste + folha para localhost: o no fixa a CA, o servidor apresenta a folha
cd "$SAIDA/dispositivos"
openssl req -x509 -newkey ec -pkeyopt ec_paramgen_curve:P-256 -nodes -days 30 \
  -subj "/CN=CA de TESTE PhxClaw" -keyout ca.key -out ca.pem 2>/dev/null
openssl req -newkey ec -pkeyopt ec_paramgen_curve:P-256 -nodes -subj "/CN=localhost" \
  -keyout srv.key -out srv.csr 2>/dev/null
printf "basicConstraints=CA:FALSE\nsubjectAltName=DNS:localhost\nextendedKeyUsage=serverAuth\n" > ext.cnf
openssl x509 -req -in srv.csr -CA ca.pem -CAkey ca.key -CAcreateserial -days 30 \
  -extfile ext.cnf -out srv.pem 2>/dev/null
openssl pkcs8 -topk8 -nocrypt -in srv.key -out srv-chave.pem
rm -f ca.key ca.srl srv.csr srv.key ext.cnf
TENANT=$(python3 -c "import uuid;print(uuid.uuid4())")
NO=$(python3 -c "import uuid;print(uuid.uuid4())")
TOKEN=$(openssl rand -hex 16)
printf "%s %s\n" "$TENANT" "$TOKEN" > tokens.txt

# Os .bat carregam os mesmos valores do tokens.txt: o dono so da dois cliques
cat > 1-servidor.bat <<EOF
@echo off
rem Deixe esta janela aberta. Servidor WSS de dispositivos em wss://localhost:8788
phxclaw.exe dispositivos --cert srv.pem --chave srv-chave.pem --tokens tokens.txt
pause
EOF
cat > 2-parear.bat <<EOF
@echo off
rem Primeira vez: pareia com o token (uso unico) e guarda a chave no Credential Manager.
set PHXCLAW_DEVICE_WSS_URL=wss://localhost:8788
set PHXCLAW_TENANT_UUID=$TENANT
set PHXCLAW_NODE_UUID=$NO
set PHXCLAW_DEVICE_CA_PEM=%~dp0ca.pem
set PHXCLAW_ENROLLMENT_TOKEN=$TOKEN
phxclaw-device-node.exe
pause
EOF
cat > 3-religar.bat <<EOF
@echo off
rem Sem token: religa pela chave guardada. Rode depois de fechar o 2-parear.
set PHXCLAW_DEVICE_WSS_URL=wss://localhost:8788
set PHXCLAW_TENANT_UUID=$TENANT
set PHXCLAW_NODE_UUID=$NO
set PHXCLAW_DEVICE_CA_PEM=%~dp0ca.pem
phxclaw-device-node.exe
pause
EOF
# cmd.exe le .bat com LF, mas o Bloco de Notas antigo mostra tudo numa linha
sed -i 's/$/\r/' ./*.bat
cd - >/dev/null

(cd "$RAIZ" && cargo run -q -p phxclaw-ui-ir --example gerar -- \
  crates/phxclaw-ui-ir/tests/fixtures/pedidos.sql Pedidos "$SAIDA/gerado" >/dev/null 2>&1)
cp "$RAIZ/crates/phxclaw-ui-ir/tests/fixtures/pedidos.sql" "$SAIDA/u4b-wlanguage/"
cp "$SAIDA/gerado/wlanguage/"* "$SAIDA/u4b-wlanguage/"
rm -rf "$SAIDA/gerado"
cp "$RAIZ/tools/kit_do_dono_LEIA-ME.md" "$SAIDA/LEIA-ME.md"
(cd "$(dirname "$SAIDA")" && rm -f "$(basename "$SAIDA").zip" && zip -qr "$(basename "$SAIDA").zip" "$(basename "$SAIDA")")
echo "kit em $SAIDA.zip ($(du -h "$SAIDA.zip" | cut -f1))"
