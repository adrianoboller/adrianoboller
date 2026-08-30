#!/usr/bin/env bash
# Prova que o binario Windows RODA -- nao que compila.
#
# A §6 do docs/EMPACOTAMENTO.md dizia, com todas as letras: "O que **nao** da:
# rodar. Sem Windows e sem `wine`, o `.exe` e conferido pela forma (PE32+
# x86-64), pelas DLLs que importa e pelos simbolos que exporta -- nunca por
# execucao." Com o `wine` instalado ela passou a poder rodar, e a fronteira
# andou o mesmo passo que o `qemu-user-static` fez andar no ARM.
#
# Nao precisa de VM: o `wine` NAO e emulador de maquina -- e uma
# reimplementacao das DLLs do Windows sobre a libc do Linux. O codigo x86-64 do
# .exe roda nativo, entao nao depende de /dev/kvm, que este ambiente nao tem.
# A consequencia importa para ler o numero: o tempo medido aqui e de codigo
# NATIVO, ao contrario do ARM, onde o qemu traduz instrucao por instrucao.
#
#   sudo apt install wine        # ~150 MB de download, ~700 MB em disco
#   bancada/windows/provar.sh
set -u
cd "$(dirname "$0")/../.." || exit 1
RAIZ=$PWD
BIN=${BIN:-$RAIZ/target/x86_64-pc-windows-gnu/release/phxsqld.exe}
W=${WINE:-wine}
PORTA=${PORTA:-7461}

command -v "$W" >/dev/null || { echo "falta $W: sudo apt install wine"; exit 2; }
# Sem o `target` (recem-limpo, por exemplo), vale o .exe DO PACOTE -- e ele e
# ate melhor: o que se prova ai e o arquivo que o usuario baixa, e nao um
# subproduto da compilacao que ninguem distribui.
if [ ! -f "$BIN" ] && [ -f "$RAIZ/pacotes"/phxsql-*-windows.zip ]; then
  Z=$(ls -1 "$RAIZ/pacotes"/phxsql-*-windows.zip | tail -1)
  D=$(mktemp -d)
  python3 -c "import sys,zipfile; zipfile.ZipFile(sys.argv[1]).extractall(sys.argv[2])" "$Z" "$D"
  BIN=$(find "$D" -name phxsqld.exe | head -1)
  echo "o binario saiu do pacote: $(basename "$Z")"
fi
[ -f "$BIN" ] || { echo "falta o binario Windows: ./empacotar.sh windows"; exit 2; }

S=$(mktemp -d); trap 'rm -rf "$S"' EXIT
# O prefixo do wine fica DENTRO do descartavel: a corrida nao deixa um ~/.wine
# com estado de outro dia para atrapalhar a proxima.
export WINEPREFIX=$S/prefixo WINEDEBUG=${WINEDEBUG:--all}
mkdir -p "$S/dados"
cd "$S" || exit 1
cp "$RAIZ/bancada/arm/sonda.py" prova.py

"$W" "$BIN" --exemplo 1 > config.json 2>/dev/null

# O PBKDF2 tambem sai do binario Windows: se a criptografia nao rodasse sob
# wine, o hash nao fecharia com o login mais abaixo.
H=$("$W" "$BIN" --senha <<< "segredo1" 2>/dev/null | grep -oE 'pbkdf2-sha256[^"]+' | head -1)
[ -n "$H" ] || { echo "o .exe nao gerou o hash da senha"; exit 1; }

# `base` fica RELATIVO de proposito: o wine mapeia Z: na raiz do Linux, entao
# caminho absoluto do Unix so funciona por acidente do drive corrente.
python3 -c "
import json
c=json.load(open('config.json'))
c['bind']='127.0.0.1:$PORTA'; c['base']='dados'
if isinstance(c.get('web'),dict): c['web']['ligado']=False
c['usuarios'][0]['login']='adm'; c['usuarios'][0]['senha_hash']='''$H'''
json.dump(c,open('config.json','w'),indent=1)
"
"$W" "$BIN" > saida.txt 2>&1 &
PID=$!
sleep 8
# O `wine` troca o processo: quem sobrevive nao e o PID do lancador, entao a
# prova de que subiu e a PORTA respondendo, nao o `kill -0`.
python3 -c "
import os, socket, sys
try:
    socket.create_connection(('127.0.0.1',$PORTA),timeout=5).close()
except OSError as e:
    print('NAO SUBIU:',e); sys.exit(1)

# O RSS sai do processo QUE ESCUTA A PORTA, achado pelo inode do soquete --
# nao do PID que o shell lancou (o wine troca o processo e aquele ja morreu),
# e nao do primeiro /proc que casa com 'phxsqld.exe' na linha de comando.
# A primeira versao fazia isso e o numero pulou de 6 MiB para 17 entre duas
# corridas iguais, porque as vezes achava o lancador do wine e as vezes o
# servidor. Numero que muda 3x sem nada mudar nao esta medindo o que diz.
#
# E o que sai INCLUI o proprio wine, do mesmo jeito que o do ARM inclui o
# qemu. Nenhum dos dois e o consumo nativo -- o nativo esta na §7.2 do
# docs/EMPACOTAMENTO.md.
alvo = '%04X' % $PORTA
inodes = set()
for linha in open('/proc/net/tcp').read().splitlines()[1:]:
    c = linha.split()
    if c[1].split(':')[1] == alvo and c[3] == '0A':   # 0A = LISTEN
        inodes.add(c[9])
if not inodes:
    print('  (nao achei quem escuta a porta)'); sys.exit(0)
for p in os.listdir('/proc'):
    if not p.isdigit(): continue
    try:
        for fd in os.listdir(f'/proc/{p}/fd'):
            if os.readlink(f'/proc/{p}/fd/{fd}') in ('socket:[%s]' % i for i in inodes):
                for l in open(f'/proc/{p}/status'):
                    if l.startswith('VmRSS:'):
                        print('  RSS sob wine:', l.split()[1], 'kB (inclui o wine)')
                        sys.exit(0)
    except OSError:
        pass
print('  (nao achei o processo dono do soquete)')
" || { tail -8 saida.txt; exit 1; }
echo "servidor Windows x86-64 no ar sob $("$W" --version 2>/dev/null)"
ALVO="Windows x86-64" MODO="sob wine" PORTA=$PORTA python3 prova.py
CODIGO=$?
kill "$PID" 2>/dev/null
wait "$PID" 2>/dev/null
"$W"server -k 2>/dev/null
exit $CODIGO
