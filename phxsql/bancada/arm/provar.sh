#!/usr/bin/env bash
# Prova que o binario ARM64 RODA -- nao que compila.
#
# A diferenca importa: a §7.3 do docs/EMPACOTAMENTO.md dizia, com todas as
# letras, que os binarios ARM nunca tinham sido executados, porque esta maquina
# e x86 e nao havia emulador. Com o `qemu-user-static` instalado ela passou a
# poder rodar, e "compilou" virou "gravou 50 linhas e leu de volta".
#
# Nao precisa de VM: `qemu-user` emula o BINARIO, nao a maquina -- entao nao
# depende de /dev/kvm, que este ambiente nao tem.
#
#   sudo apt install qemu-user-static
#   bancada/arm/provar.sh
set -u
cd "$(dirname "$0")/../.." || exit 1
RAIZ=$PWD
BIN=$RAIZ/target/aarch64-unknown-linux-musl/release/phxsqld
Q=${QEMU:-qemu-aarch64-static}
PORTA=${PORTA:-6992}

command -v "$Q" >/dev/null || { echo "falta $Q: sudo apt install qemu-user-static"; exit 2; }
[ -x "$BIN" ] || { echo "falta o binario ARM: ./empacotar.sh arm64"; exit 2; }

S=$(mktemp -d); trap 'rm -rf "$S"' EXIT
mkdir -p "$S/dados"; cd "$S" || exit 1
cp "$RAIZ/bancada/arm/sonda.py" prova.py
$Q "$BIN" --exemplo 1 > config.json

# O PBKDF2 tambem sai do binario emulado: se a criptografia nao rodasse em ARM,
# o hash nao fecharia com o login mais abaixo.
H=$($Q "$BIN" --senha <<< "segredo1" | grep -oE 'pbkdf2-sha256[^"]+' | head -1)
python3 -c "
import json
c=json.load(open('config.json'))
c['bind']='127.0.0.1:$PORTA'; c['base']='$S/dados'
if isinstance(c.get('web'),dict): c['web']['ligado']=False
c['usuarios'][0]['login']='adm'; c['usuarios'][0]['senha_hash']='''$H'''
json.dump(c,open('config.json','w'),indent=1)
"
$Q "$BIN" > saida.txt 2>&1 &
PID=$!
sleep 6
kill -0 "$PID" 2>/dev/null || { echo "NAO SUBIU:"; tail -6 saida.txt; exit 1; }
echo "servidor ARM64 no ar sob emulacao, RSS $(awk '/VmRSS/{print $2}' /proc/$PID/status) kB"
PORTA=$PORTA python3 prova.py
CODIGO=$?
kill "$PID" 2>/dev/null; wait "$PID" 2>/dev/null
exit $CODIGO
