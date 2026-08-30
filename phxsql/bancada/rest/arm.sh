#!/usr/bin/env bash
# Prova que o webservice REST RODA em ARM64 -- nao que compila.
#
# A diferenca e a mesma que o `bancada/arm/provar.sh` ja pagou: "compilou" nao
# e "atendeu um pedido". Aqui o binario aarch64 sobe sob `qemu-aarch64-static`,
# abre a porta REST e responde a um pedido HTTP montado byte a byte.
#
# `qemu-user` emula o BINARIO, nao a maquina -- entao nao depende de /dev/kvm,
# que este ambiente nao tem.
#
#   sudo apt install qemu-user-static
#   CARGO_TARGET_AARCH64_UNKNOWN_LINUX_MUSL_LINKER=rust-lld \
#     cargo build --release --offline --target aarch64-unknown-linux-musl \
#     -p phxsql-server --bin phxsqld
#   bancada/rest/arm.sh
set -u
cd "$(dirname "$0")/../.." || exit 1
RAIZ=$PWD
BIN=$RAIZ/target/aarch64-unknown-linux-musl/release/phxsqld
Q=${QEMU:-qemu-aarch64-static}
PORTA_DADOS=${PORTA_DADOS:-7520}
PORTA_REST=${PORTA_REST:-7521}
PORTA_SWAGGER=${PORTA_SWAGGER:-7522}
TOKEN=armrest

command -v "$Q" >/dev/null || { echo "falta $Q: sudo apt install qemu-user-static"; exit 2; }
[ -x "$BIN" ] || { echo "falta o binario ARM64 -- ver o cabecalho deste script"; exit 2; }

S=$(mktemp -d); trap 'rm -rf "$S"' EXIT
mkdir -p "$S/dados"; cd "$S" || exit 1

# O PBKDF2 tambem sai do binario emulado: se a criptografia nao rodasse em ARM,
# o hash nao fecharia com o login mais abaixo.
H=$($Q "$BIN" --senha <<< "segredo1" | grep -oE 'pbkdf2-sha256[^"]+' | head -1)
python3 - "$PORTA_DADOS" "$PORTA_REST" "$PORTA_SWAGGER" "$TOKEN" "$H" "$S" <<'PY'
import json, sys
dados, rest, swagger, token, h, s = sys.argv[1:7]
json.dump({
  "bind": f"127.0.0.1:{dados}",
  "base": f"{s}/dados",
  "token": token,
  "web": {"ligado": False},
  "rest": {"ligado": True, "bind": f"127.0.0.1:{rest}", "nome": "placa",
           "swagger_ligado": True, "swagger_bind": f"127.0.0.1:{swagger}"},
  "usuarios": [{"login": "adm", "nome": "Adm", "id": 1, "senha_hash": h,
                "bases": {"*": {"ler": True, "inserir": True, "criar": True,
                                "administrar": True}}}],
}, open("config.json", "w"), indent=1)
PY

$Q "$BIN" > saida.txt 2>&1 &
PID=$!
sleep 8
kill -0 "$PID" 2>/dev/null || { echo "NAO SUBIU:"; tail -8 saida.txt; exit 1; }
echo "phxsqld ARM64 no ar sob emulacao, RSS $(awk '/VmRSS/{print $2}' /proc/$PID/status) kB"

PORTA_REST=$PORTA_REST PORTA_SWAGGER=$PORTA_SWAGGER TOKEN=$TOKEN python3 - <<'PY'
import json, os, socket, sys

def http(porta, metodo, caminho, corpo=None, token=None):
    dados = b"" if corpo is None else json.dumps(corpo).encode()
    linhas = [f"{metodo} {caminho} HTTP/1.1", "Host: 127.0.0.1", "Connection: close"]
    if token:
        linhas.append(f"Authorization: Bearer {token}")
    if corpo is not None:
        linhas.append("Content-Type: application/json")
        linhas.append(f"Content-Length: {len(dados)}")
    s = socket.create_connection(("127.0.0.1", porta), 15)
    s.settimeout(15)
    s.sendall(("\r\n".join(linhas) + "\r\n\r\n").encode() + dados)
    bruto = b""
    while True:
        p = s.recv(65536)
        if not p:
            break
        bruto += p
    s.close()
    cabeca, _, resto = bruto.partition(b"\r\n\r\n")
    codigo = int(cabeca.split()[1])
    return codigo, resto.decode("utf-8", "replace")

rest = int(os.environ["PORTA_REST"])
swagger = int(os.environ["PORTA_SWAGGER"])
token = os.environ["TOKEN"]
falhas = 0

def ok(nome, cond, detalhe=""):
    global falhas
    print(("  OK   " if cond else "  FALHA") + f"  {nome}" + (f"  -- {detalhe}" if detalhe else ""))
    if not cond:
        falhas += 1

c, corpo = http(rest, "POST", "/v1/ping", {}, token)
r = json.loads(corpo)
ok("o REST responde em ARM64", c == 200 and r.get("ok"), corpo[:120])
c, corpo = http(rest, "GET", "/openapi.json")
espec = json.loads(corpo)
ok("a especificacao e gerada na placa", c == 200 and len(espec.get("paths", {})) > 100,
   f"{len(espec.get('paths', {}))} rotas, {len(corpo)} bytes")
c, corpo = http(swagger, "GET", "/")
ok("o explorador desenha em ARM64", c == 200 and "<title>placa</title>" in corpo,
   str(c))
c, corpo = http(rest, "POST", "/v1/ping", {}, "chute")
ok("e o portao continua fechando", c == 401, str(c))
sys.exit(1 if falhas else 0)
PY
CODIGO=$?
kill "$PID" 2>/dev/null; wait "$PID" 2>/dev/null
exit $CODIGO
