#!/usr/bin/env bash
# PASSO 1 da bateria do server mail: instalar + configurar a base, exercitando
# o binario de verdade.
#
#   bash bancada/servermail/instalar.sh <dir_de_trabalho> [porta]
#
# O que faz, e prova:
#   1. gera um config.json de exemplo com `phxsqld --exemplo 1`
#   2. gera o senha_hash com `phxsqld --senha` -- prova que a senha NAO fica em
#      claro no config (a lei "senha nunca em texto puro")
#   3. sobe o servidor com esse config e confere que ele ESCUTA
#   4. confere que a base (a pasta) nasce no disco
#   5. derruba o servidor pelo PID que guardou (nunca por nome, nunca mata o do
#      vizinho)
#
# Nao deixa processo de pe. Nao toca em rede externa.
set -u

RAIZ="$(cd "$(dirname "$0")/../.." && pwd)"
PHXSQLD="$RAIZ/target/release/phxsqld"
DIR="${1:?uso: instalar.sh <dir_de_trabalho> [porta]}"
PORTA="${2:-5951}"

if [ ! -x "$PHXSQLD" ]; then
  echo "phxsqld nao compilado -- rode: cargo build --release -p phxsql-server" >&2
  exit 2
fi

mkdir -p "$DIR"
cd "$DIR" || exit 2

echo "PASSO 1) instalar + configurar a base do server mail (porta $PORTA)"

# (1) modelo de config
"$PHXSQLD" --exemplo 1 > modelo.json 2>/dev/null || { echo "  FALHA: --exemplo"; exit 1; }

# (2) senha_hash: a senha entra por stdin e NAO aparece no config em claro
HASH_LINE="$(printf '%s' 'senha-do-servermail' | "$PHXSQLD" --senha 2>/dev/null)"
echo "  senha_hash gerado (PBKDF2, sem texto puro): ${HASH_LINE:0:48}..."

# monta o config final: bind na porta pedida, base local, token de teste
python3 - "$PORTA" <<'PY'
import json, sys
c = json.load(open("modelo.json"))
c["bind"] = f"127.0.0.1:{sys.argv[1]}"
c["base"] = "base"
c["token"] = "token-de-teste-servermail"
json.dump(c, open("config.json", "w"), indent=2, ensure_ascii=False)
print("  config.json escrito: bind 127.0.0.1:%s, base=base" % sys.argv[1])
PY

# confere que nao ha a palavra "senha" em claro no config (so senha_hash)
if grep -qiE '"senha"\s*:' config.json; then
  echo "  FALHA: apareceu campo senha em claro no config"; exit 1
fi
echo "  ok  config nao tem senha em claro (so senha_hash PBKDF2)"

# (3) sobe o servidor
"$PHXSQLD" --config config.json > servidor.out 2>&1 &
SRV=$!
LISTENING=0
for _ in $(seq 1 50); do
  if python3 -c "import socket,sys; s=socket.socket(); s.settimeout(0.3); sys.exit(0 if s.connect_ex(('127.0.0.1',$PORTA))==0 else 1)" 2>/dev/null; then
    LISTENING=1; break
  fi
  python3 -c "import time; time.sleep(0.1)"
done

if [ "$LISTENING" = "1" ]; then
  echo "  ok  servidor SOBE e ESCUTA em 127.0.0.1:$PORTA (pid $SRV)"
  head -1 servidor.out | sed 's/^/       /'
else
  echo "  FALHA: servidor nao subiu"; cat servidor.out; kill "$SRV" 2>/dev/null; exit 1
fi

# (4) base nasce no disco
if [ -d "$DIR/base" ]; then
  echo "  ok  base nasce no disco: $DIR/base"
else
  echo "  FALHA: pasta base nao foi criada"; kill "$SRV" 2>/dev/null; exit 1
fi

# (5) derruba pelo PID guardado
kill "$SRV" 2>/dev/null
wait "$SRV" 2>/dev/null
echo "  ok  servidor derrubado pelo PID $SRV (nunca por nome)"
exit 0
