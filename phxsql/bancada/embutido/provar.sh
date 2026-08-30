#!/usr/bin/env bash
# Prova que o PhxSql EMBUTIDO roda ligado a um programa em C -- em x86-64 e,
# sob emulacao, em ARM64.
#
# "Compila" nao e "roda", e esta casa ja pagou essa licao uma vez: a §7.3 do
# docs/EMPACOTAMENTO.md dizia com todas as letras que os binarios ARM nunca
# tinham sido executados. O `qemu-user-static` emula o BINARIO, nao a maquina,
# e nao depende de /dev/kvm -- que este ambiente nao tem.
#
#   bancada/embutido/provar.sh          # os dois
#   bancada/embutido/provar.sh x86      # so o nativo
#
# # Por que o ARM linka a mao em vez de chamar o `cc`
#
# Nao ha compilador cruzado de C nesta maquina, e nao ha sysroot de aarch64.
# O que ha e (a) o `clang`, que e cruzado por natureza, (b) o `ld.lld`, e (c) a
# libc musl de aarch64 que o proprio rustup ja instalou junto com o alvo
# `aarch64-unknown-linux-musl`. Com `-nostdlibinc` o clang usa so os
# cabecalhos DELE (stdint.h, stddef.h), que sao por-alvo -- e o `prova.c`
# declara a mao a unica funcao de libc que usa, justamente para caber nisso.
# O fonte e o MESMO nos dois lados, que e o que faz a prova valer.
set -u
cd "$(dirname "$0")/../.." || exit 1
RAIZ=$PWD
QUAL=${1:-tudo}
S=$(mktemp -d) || exit 1
trap 'rm -rf "$S"' EXIT

INC=$RAIZ/crates/phxsql-ffi/include
FONTE=$RAIZ/crates/phxsql-ffi/c/prova.c
FALHAS=0

linha() { printf '\n\033[1m== %s ==\033[0m\n' "$1"; }

# ---------------------------------------------------------------- x86-64
if [ "$QUAL" = tudo ] || [ "$QUAL" = x86 ]; then
  linha "x86-64 nativo"
  cargo build --release -p phxsql-ffi --offline || exit 1
  A=$RAIZ/target/release/libphxsql_ffi.a
  SO=$RAIZ/target/release/libphxsql_ffi.so
  echo "staticlib $(du -h "$A"  | cut -f1)   cdylib $(du -h "$SO" | cut -f1)"
  echo "simbolos phx_ exportados no .so: $(nm -D --defined-only "$SO" | grep -c ' T phx_')"

  cc -std=c11 -Wall -Wextra -O2 -I"$INC" "$FONTE" "$A" \
     -o "$S/prova-x86" -lpthread -ldl -lm || { echo "nao compilou"; exit 1; }
  echo "programa em C: $(du -h "$S/prova-x86" | cut -f1) (ligado ESTATICAMENTE ao motor)"
  mkdir -p "$S/dados-x86"
  "$S/prova-x86" "$S/dados-x86" || FALHAS=$((FALHAS + 1))

  # O mesmo programa contra a cdylib, que e o formato do Android.
  cc -std=c11 -O2 -I"$INC" "$FONTE" -L"$RAIZ/target/release" -lphxsql_ffi \
     -o "$S/prova-so" || { echo "nao ligou contra a cdylib"; exit 1; }
  mkdir -p "$S/dados-so"
  LD_LIBRARY_PATH=$RAIZ/target/release "$S/prova-so" "$S/dados-so" >"$S/so.txt" 2>&1
  if [ $? -eq 0 ]; then
    echo "e contra a cdylib (o formato do Android): $(tail -1 "$S/so.txt")"
  else
    echo "FALHOU contra a cdylib:"; tail -20 "$S/so.txt"; FALHAS=$((FALHAS + 1))
  fi
fi

# ----------------------------------------------------------------- ARM64
if [ "$QUAL" = tudo ] || [ "$QUAL" = arm ]; then
  linha "ARM64 sob qemu-aarch64-static"
  Q=${QEMU:-qemu-aarch64-static}
  command -v "$Q"      >/dev/null || { echo "falta $Q: apt install qemu-user-static"; exit 2; }
  command -v clang     >/dev/null || { echo "falta o clang"; exit 2; }
  command -v ld.lld    >/dev/null || { echo "falta o ld.lld"; exit 2; }

  ALVO=aarch64-unknown-linux-musl
  SYS=$(rustc --print sysroot)/lib/rustlib/$ALVO/lib/self-contained
  [ -d "$SYS" ] || { echo "falta o alvo $ALVO: rustup target add $ALVO"; exit 2; }

  cargo build --release -p phxsql-ffi --offline --target "$ALVO" || exit 1
  AARM=$RAIZ/target/$ALVO/release/libphxsql_ffi.a
  echo "staticlib ARM64: $(du -h "$AARM" | cut -f1)"

  clang -target aarch64-unknown-linux-musl -nostdlibinc -DPHX_SEM_CABECALHOS \
        -std=c11 -Wall -Wextra -O2 -I"$INC" -c "$FONTE" -o "$S/prova-arm.o" \
    || { echo "nao compilou para ARM"; exit 1; }

  # Estatico: crt do musl + o nosso .a + a libc e o desenrolador do proprio
  # alvo. Sem carregador dinamico, que e o que faz um .a de iOS ser plausivel.
  #
  # `--eh-frame-hdr` NAO e enfeite, e esta linha custou uma rodada vermelha:
  # sem ele o `ld.lld` nao emite o PT_GNU_EH_FRAME, o desenrolador nao acha a
  # tabela de FDE, e todo `catch_unwind` vira
  #
  #     fatal runtime error: failed to initiate panic, error 5, aborting
  #
  # Ou seja: a garantia central desta camada -- nenhum panico atravessa --
  # some CALADA por causa de uma bandeira do ligador, e o sintoma e o app do
  # cliente abortando. O `cc` e o `clang` passam essa bandeira sozinhos; quem
  # chama o ligador na mao, como o `ld` de um projeto de iOS ou de Android
  # pode acabar fazendo, tem de passar. Esta escrito em docs/EMBUTIDO.md.
  ld.lld -static --eh-frame-hdr -o "$S/prova-arm" \
        "$SYS/crt1.o" "$SYS/crti.o" "$S/prova-arm.o" \
        "$AARM" "$SYS/libc.a" "$SYS/libunwind.a" "$SYS/crtn.o" \
    || { echo "nao ligou para ARM"; exit 1; }

  file "$S/prova-arm" 2>/dev/null | sed 's/^/  /'
  echo "programa em C ARM64: $(du -h "$S/prova-arm" | cut -f1)"
  mkdir -p "$S/dados-arm"
  "$Q" "$S/prova-arm" "$S/dados-arm" || FALHAS=$((FALHAS + 1))
fi

linha "resultado"
if [ "$FALHAS" -eq 0 ]; then
  echo "tudo passou"
else
  echo "$FALHAS rodada(s) com falha"
fi
exit "$FALHAS"
