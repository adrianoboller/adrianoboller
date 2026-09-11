#!/usr/bin/env bash
# Compila a micro-bancada do Registro para Windows, CRUZADO, aqui no Linux.
#
# Este script NAO roda a bancada -- ele so a compila. O binario roda num
# Windows de verdade (ver LEIA-ME.md): sob wine, o resultado seria do wine e
# nao do Configuration Manager do kernel do Windows.
#
# Zero dependencia: -ladvapi32 e -lkernel32 sao DLLs do proprio Windows.
set -euo pipefail
cd "$(dirname "$0")"

CC=${CC:-x86_64-w64-mingw32-gcc}
OBJDUMP=${OBJDUMP:-x86_64-w64-mingw32-objdump}

if ! command -v "$CC" >/dev/null 2>&1; then
    echo "falta o compilador cruzado: $CC"
    echo "  Debian/Ubuntu: apt install mingw-w64 | Fedora: mingw64-gcc | Arch: mingw-w64-gcc"
    exit 1
fi

"$CC" -O2 -Wall -o bench-registro.exe bench.c -ladvapi32
echo "gerado: $(pwd)/bench-registro.exe"

file bench-registro.exe

echo "-- DLLs que o .exe importa (tem de ser SO do Windows) --"
if command -v "$OBJDUMP" >/dev/null 2>&1; then
    "$OBJDUMP" -p bench-registro.exe | grep -i 'DLL Name' || true
    echo "(se aparecer libgcc/libwinpthread, algo puxou runtime do mingw -- nao deveria)"
fi
