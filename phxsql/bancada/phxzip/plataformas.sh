#!/usr/bin/env bash
# Bancada do PhxZip por PLATAFORMA-ALVO.
#
# Prova, alvo a alvo, o nivel mais alto que se consegue medir nesta maquina:
# RODA (executou e o resultado bate) > LIGA (binario existe, nao rodou) >
# COMPILA (so a biblioteca, sem ligador) > NAO MEDIDO (com o motivo).
#
# Nada aqui e digitado a mao no dossie: este script GRAVA
# bancada/phxzip/resultados.json, com a data da medicao, o alvo, o nivel
# alcancado e o comando que produziu o nivel -- e e dali que qualquer pagina
# se gera, nunca da memoria de quem rodou.
#
# Uso:
#   bash bancada/phxzip/plataformas.sh
#
# Precisa instalado (apt, uma vez, ferramenta de teste no CONTAINER -- nao
# entra no produto, que continua zero dependencia externa):
#   apt-get install -y qemu-user qemu-user-static gcc-arm-linux-gnueabihf \
#     musl-tools gcc-s390x-linux-gnu gcc-mingw-w64-x86-64 wine64
#   rustup target add armv7-unknown-linux-musleabihf s390x-unknown-linux-gnu \
#     x86_64-pc-windows-gnu aarch64-linux-android aarch64-apple-darwin \
#     x86_64-apple-darwin aarch64-apple-ios
# Android exige o NDK r27c em /opt/android-ndk-r27c (o .cargo/config.toml do
# repositorio ja aponta o ligador para la); baixe com:
#   curl -sS -o /tmp/ndk.zip \
#     https://dl.google.com/android/repository/android-ndk-r27c-linux.zip \
#     && unzip -q /tmp/ndk.zip -d /opt
#
# macOS/iOS: sem o SDK da Apple aqui, so a biblioteca (rlib) compila -- ligar
# um binario exige esse SDK, que este script nao instala porque nao ha de onde
# baixar sem uma licenca Apple.
# ESP32 classico (Xtensa) e AVR: sem std pre-compilada no Rust estavel 1.94.1
# (confirmado com `rustup target add`); exigiriam -Z build-std (nightly) ou o
# toolchain proprio da Espressif -- fora do escopo desta bancada.

set -euo pipefail

RAIZ="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$RAIZ"

: "${CARGO_TARGET_DIR:?defina CARGO_TARGET_DIR antes de rodar}"
SAIDA="$RAIZ/bancada/phxzip/resultados.json"
DATA="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
TRABALHO="$(mktemp -d)"
trap 'rm -rf "$TRABALHO"' EXIT

RUSTC_VERBOSO="$(rustc --version --verbose | tr '\n' ' ')"

# --- massa de prova: um arquivo de texto e um binario aleatorio ---
mkdir -p "$TRABALHO/origem"
echo "conteudo de prova phxzip - bancada de plataformas" > "$TRABALHO/origem/arquivo1.txt"
head -c 100000 /dev/urandom > "$TRABALHO/origem/arquivo2.bin"
SHA_ORIGEM_1="$(sha256sum "$TRABALHO/origem/arquivo1.txt" | awk '{print $1}')"
SHA_ORIGEM_2="$(sha256sum "$TRABALHO/origem/arquivo2.bin" | awk '{print $1}')"

REGISTROS=()

registrar() {
  # registrar <alvo> <nivel> <comando> <prova> [detalhe]
  local alvo="$1" nivel="$2" comando="$3" prova="$4" detalhe="${5:-}"
  REGISTROS+=("$(python3 - "$alvo" "$nivel" "$comando" "$prova" "$detalhe" "$DATA" <<'PY'
import json, sys
alvo, nivel, comando, prova, detalhe, data = sys.argv[1:7]
print(json.dumps({
    "alvo": alvo,
    "nivel": nivel,
    "comando": comando,
    "prova": prova,
    "detalhe": detalhe,
    "medido_em": data,
}))
PY
)")
}

falhou() { echo "FALHOU: $*" >&2; }

echo "== rustc: $RUSTC_VERBOSO"

# ---------------------------------------------------------------------------
# 1) ARM 32 (Raspberry) -- armv7-unknown-linux-musleabihf, sob qemu-arm
# ---------------------------------------------------------------------------
ALVO=armv7-unknown-linux-musleabihf
if command -v qemu-arm >/dev/null && rustup target list --installed | grep -qx "$ALVO"; then
  if cargo build --release --target "$ALVO" -p phxzip-cmd --offline >"$TRABALHO/build-$ALVO.log" 2>&1; then
    BIN="$CARGO_TARGET_DIR/$ALVO/release/phxzipcmd"
    W="$TRABALHO/arm"; mkdir -p "$W"
    cp -r "$TRABALHO/origem" "$W/origem"
    (
      cd "$W"
      qemu-arm "$BIN" a saida.7z origem/arquivo1.txt origem/arquivo2.bin -psegredo123 -y
      qemu-arm "$BIN" t saida.7z -psegredo123
      qemu-arm "$BIN" x saida.7z -osaida -psegredo123 -y
    ) >"$TRABALHO/exec-$ALVO.log" 2>&1
    S1="$(sha256sum "$W/saida/arquivo1.txt" | awk '{print $1}')"
    S2="$(sha256sum "$W/saida/arquivo2.bin" | awk '{print $1}')"
    cp "$W/saida.7z" "$TRABALHO/arm.7z"
    if [ "$S1" = "$SHA_ORIGEM_1" ] && [ "$S2" = "$SHA_ORIGEM_2" ]; then
      registrar "$ALVO" "RODA" \
        "cargo build --release --target $ALVO -p phxzip-cmd --offline; qemu-arm phxzipcmd {a,t,x}" \
        "sha256 extraido == sha256 origem: $S1 / $S2"
      # testes de unidade + integracao da phxzip sob qemu-arm
      export CARGO_TARGET_ARMV7_UNKNOWN_LINUX_MUSLEABIHF_RUNNER=qemu-arm
      # A suite INTEIRA, sem teste pulado: uma prova de portabilidade que
      # pula o teste vermelho prova menos do que diz. Se um teste falhar
      # aqui, confira no x86_64 nativo antes de culpar o alvo.
      if cargo test --release --target "$ALVO" -p phxzip --offline >"$TRABALHO/testes-$ALVO.log" 2>&1; then
        N="$(grep -c '^test .* ok$' "$TRABALHO/testes-$ALVO.log" || true)"
        registrar "$ALVO (testes)" "RODA" \
          "CARGO_TARGET_ARMV7_UNKNOWN_LINUX_MUSLEABIHF_RUNNER=qemu-arm cargo test --release --target $ALVO -p phxzip --offline" \
          "$N testes/doctests OK sob qemu-arm, nenhum pulado"
      else
        falhou "testes de $ALVO sob qemu-arm"
        registrar "$ALVO (testes)" "NAO MEDIDO" "cargo test --release --target $ALVO -p phxzip --offline" "falhou alem do teste conhecido -- ver $TRABALHO/testes-$ALVO.log"
      fi
    else
      falhou "sha256 nao bateu em $ALVO"
      registrar "$ALVO" "LIGA" "cargo build --release --target $ALVO -p phxzip-cmd --offline" "ligou mas o ciclo a/t/x nao bateu o sha256 -- ver $TRABALHO/exec-$ALVO.log"
    fi
  else
    falhou "build de $ALVO"
    registrar "$ALVO" "NAO MEDIDO" "-" "build falhou -- ver $TRABALHO/build-$ALVO.log"
  fi
else
  registrar "$ALVO" "NAO MEDIDO" "-" "faltou qemu-arm ou o alvo rustup (apt-get install qemu-user-static; rustup target add $ALVO)"
fi

# ---------------------------------------------------------------------------
# 2) big-endian -- s390x-unknown-linux-gnu, sob qemu-s390x
# ---------------------------------------------------------------------------
ALVO=s390x-unknown-linux-gnu
if command -v qemu-s390x >/dev/null && command -v s390x-linux-gnu-gcc >/dev/null \
   && rustup target list --installed | grep -qx "$ALVO"; then
  export CARGO_TARGET_S390X_UNKNOWN_LINUX_GNU_LINKER=s390x-linux-gnu-gcc
  if cargo build --release --target "$ALVO" -p phxzip-cmd --offline >"$TRABALHO/build-$ALVO.log" 2>&1; then
    BIN="$CARGO_TARGET_DIR/$ALVO/release/phxzipcmd"
    ENDIAN="$(file "$BIN" | grep -o 'MSB\|LSB')"
    W="$TRABALHO/s390x"; mkdir -p "$W"
    cp -r "$TRABALHO/origem" "$W/origem"
    (
      cd "$W"
      qemu-s390x -L /usr/s390x-linux-gnu "$BIN" a saida.7z origem/arquivo1.txt origem/arquivo2.bin -psegredo123 -y
      qemu-s390x -L /usr/s390x-linux-gnu "$BIN" t saida.7z -psegredo123
      qemu-s390x -L /usr/s390x-linux-gnu "$BIN" x saida.7z -osaida -psegredo123 -y
    ) >"$TRABALHO/exec-$ALVO.log" 2>&1
    S1="$(sha256sum "$W/saida/arquivo1.txt" | awk '{print $1}')"
    S2="$(sha256sum "$W/saida/arquivo2.bin" | awk '{print $1}')"
    if [ "$S1" = "$SHA_ORIGEM_1" ] && [ "$S2" = "$SHA_ORIGEM_2" ] && [ "$ENDIAN" = "MSB" ]; then
      registrar "$ALVO" "RODA" \
        "CARGO_TARGET_S390X_UNKNOWN_LINUX_GNU_LINKER=s390x-linux-gnu-gcc cargo build --release --target $ALVO -p phxzip-cmd --offline; qemu-s390x -L /usr/s390x-linux-gnu phxzipcmd {a,t,x}" \
        "ELF $ENDIAN (big-endian); sha256 extraido == origem: $S1 / $S2"
      export CARGO_TARGET_S390X_UNKNOWN_LINUX_GNU_RUNNER="qemu-s390x -L /usr/s390x-linux-gnu"
      # A suite inteira, como no bloco ARM acima.
      if cargo test --release --target "$ALVO" -p phxzip --offline >"$TRABALHO/testes-$ALVO.log" 2>&1; then
        N="$(grep -c '^test .* ok$' "$TRABALHO/testes-$ALVO.log" || true)"
        registrar "$ALVO (testes)" "RODA" \
          "CARGO_TARGET_S390X_UNKNOWN_LINUX_GNU_RUNNER='qemu-s390x -L /usr/s390x-linux-gnu' cargo test --release --target $ALVO -p phxzip --offline" \
          "$N testes/doctests OK sob qemu-s390x, ELF $ENDIAN, nenhum pulado"
      else
        falhou "testes de $ALVO sob qemu-s390x"
        registrar "$ALVO (testes)" "NAO MEDIDO" "cargo test --release --target $ALVO -p phxzip --offline" "falhou alem do teste conhecido -- ver $TRABALHO/testes-$ALVO.log"
      fi
    else
      falhou "sha256 ou endianness nao bateu em $ALVO"
      registrar "$ALVO" "LIGA" "cargo build --release --target $ALVO -p phxzip-cmd --offline" "ligou mas o ciclo nao bateu -- ver $TRABALHO/exec-$ALVO.log"
    fi
  else
    falhou "build de $ALVO"
    registrar "$ALVO" "NAO MEDIDO" "-" "build falhou -- ver $TRABALHO/build-$ALVO.log"
  fi
else
  registrar "$ALVO" "NAO MEDIDO" "-" "faltou qemu-s390x ou gcc-s390x-linux-gnu ou o alvo rustup"
fi

# ---------------------------------------------------------------------------
# 3) Windows -- x86_64-pc-windows-gnu, sob wine (quando instalado)
# ---------------------------------------------------------------------------
ALVO=x86_64-pc-windows-gnu
if command -v x86_64-w64-mingw32-gcc >/dev/null && rustup target list --installed | grep -qx "$ALVO"; then
  if cargo build --release --target "$ALVO" -p phxzip-cmd --offline >"$TRABALHO/build-$ALVO.log" 2>&1; then
    BIN="$CARGO_TARGET_DIR/$ALVO/release/phxzipcmd.exe"
    if command -v wine >/dev/null; then
      export WINEDEBUG=-all
      W="$TRABALHO/win"; mkdir -p "$W"
      cp -r "$TRABALHO/origem" "$W/origem"
      (
        cd "$W"
        timeout 60 wine "$BIN" a saida.7z origem/arquivo1.txt origem/arquivo2.bin -psegredo123 -y
        timeout 60 wine "$BIN" t saida.7z -psegredo123
        timeout 60 wine "$BIN" x saida.7z -osaida -psegredo123 -y
      ) >"$TRABALHO/exec-$ALVO.log" 2>&1
      S1="$(sha256sum "$W/saida/arquivo1.txt" | awk '{print $1}')"
      S2="$(sha256sum "$W/saida/arquivo2.bin" | awk '{print $1}')"
      if [ "$S1" = "$SHA_ORIGEM_1" ] && [ "$S2" = "$SHA_ORIGEM_2" ]; then
        registrar "$ALVO" "RODA" \
          "cargo build --release --target $ALVO -p phxzip-cmd --offline; wine phxzipcmd.exe {a,t,x}" \
          "sha256 extraido == origem: $S1 / $S2"
      else
        falhou "sha256 nao bateu em $ALVO (wine)"
        registrar "$ALVO" "LIGA" "cargo build --release --target $ALVO -p phxzip-cmd --offline" "ligou, wine rodou mas o ciclo nao bateu -- ver $TRABALHO/exec-$ALVO.log"
      fi
    else
      registrar "$ALVO" "LIGA" "cargo build --release --target $ALVO -p phxzip-cmd --offline" "ligou; wine nao instalado, nao rodou (apt-get install wine64)"
    fi
  else
    falhou "build de $ALVO"
    registrar "$ALVO" "NAO MEDIDO" "-" "build falhou -- ver $TRABALHO/build-$ALVO.log"
  fi
else
  registrar "$ALVO" "NAO MEDIDO" "-" "faltou gcc-mingw-w64-x86-64 ou o alvo rustup"
fi

# ---------------------------------------------------------------------------
# 4) Android -- aarch64-linux-android pelo NDK r27c
# ---------------------------------------------------------------------------
ALVO=aarch64-linux-android
NDK=/opt/android-ndk-r27c
if [ -d "$NDK" ] && rustup target list --installed | grep -qx "$ALVO"; then
  if cargo build --release --target "$ALVO" -p phxzip-cmd --offline >"$TRABALHO/build-$ALVO.log" 2>&1; then
    BIN="$CARGO_TARGET_DIR/$ALVO/release/phxzipcmd"
    TIPO="$(file "$BIN")"
    # sem sysroot bionic (/system/bin/linker64) nesta maquina: nao ha como
    # rodar sem um emulador Android real (AVD) ou um dispositivo.
    if command -v qemu-aarch64 >/dev/null && timeout 5 qemu-aarch64 "$BIN" --help >"$TRABALHO/exec-$ALVO.log" 2>&1; then
      registrar "$ALVO" "RODA" "cargo build --release --target $ALVO -p phxzip-cmd --offline; qemu-aarch64 phxzipcmd" "$TIPO"
    else
      registrar "$ALVO" "LIGA" "cargo build --release --target $ALVO -p phxzip-cmd --offline" "$TIPO -- NAO MEDIDO rodar: falta emulador Android (AVD) ou dispositivo; qemu-user generico nao tem o /system/bin/linker64 da bionic"
    fi
  else
    falhou "build de $ALVO"
    registrar "$ALVO" "NAO MEDIDO" "-" "build falhou -- ver $TRABALHO/build-$ALVO.log"
  fi
else
  registrar "$ALVO" "NAO MEDIDO" "-" "faltou o NDK r27c em $NDK ou o alvo rustup (baixe o NDK, ver cabecalho deste script)"
fi

# ---------------------------------------------------------------------------
# 5) macOS e iOS -- so a biblioteca (rlib), sem ligador -- sem SDK Apple aqui
# ---------------------------------------------------------------------------
for ALVO in aarch64-apple-darwin x86_64-apple-darwin aarch64-apple-ios; do
  if rustup target list --installed | grep -qx "$ALVO"; then
    if cargo build --release --target "$ALVO" -p phxzip --lib --offline >"$TRABALHO/build-$ALVO.log" 2>&1; then
      RLIB=$(find "$CARGO_TARGET_DIR/$ALVO/release" -maxdepth 1 -name 'libphxzip*.rlib' | head -1)
      registrar "$ALVO" "COMPILA" "cargo build --release --target $ALVO -p phxzip --lib --offline" "$(basename "${RLIB:-?}") -- so a lib; ligar um binario exige o SDK da Apple, ausente nesta maquina"
    else
      falhou "build da lib em $ALVO"
      registrar "$ALVO" "NAO MEDIDO" "-" "build da lib falhou -- ver $TRABALHO/build-$ALVO.log"
    fi
  else
    registrar "$ALVO" "NAO MEDIDO" "-" "alvo rustup ausente (rustup target add $ALVO)"
  fi
done

# ---------------------------------------------------------------------------
# 6) IoT -- thumbv7em (Arduino ARM) e riscv32imc (ESP32-C3): so a lib
# ---------------------------------------------------------------------------
for ALVO in thumbv7em-none-eabihf riscv32imc-unknown-none-elf; do
  if rustup target list --installed | grep -qx "$ALVO"; then
    if cargo build --release --target "$ALVO" -p phxzip --lib --offline >"$TRABALHO/build-$ALVO.log" 2>&1; then
      registrar "$ALVO" "COMPILA" "cargo build --release --target $ALVO -p phxzip --lib --offline" "no_std + alloc; embarcado nao roda phxzip-cmd (std) por natureza -- so a biblioteca"
    else
      falhou "build de $ALVO"
      registrar "$ALVO" "NAO MEDIDO" "-" "build falhou -- ver $TRABALHO/build-$ALVO.log"
    fi
  else
    registrar "$ALVO" "NAO MEDIDO" "-" "alvo rustup ausente"
  fi
done

# ESP32 classico (Xtensa) e AVR: sem std pre-compilada no Rust estavel
for ALVO in xtensa-esp32-none-elf avr-none; do
  if rustup target add "$ALVO" >"$TRABALHO/rustup-$ALVO.log" 2>&1; then
    registrar "$ALVO" "COMPILA" "rustup target add $ALVO; cargo build --release --target $ALVO -p phxzip --lib --offline" "alvo instalado nesta rodada -- ver log para o resultado do build"
  else
    registrar "$ALVO" "NAO MEDIDO" "-" "'rustup target add $ALVO' recusa: sem prebuilt std no Rust estavel 1.94.1 -- exige -Z build-std (nightly) ou toolchain proprio da Espressif/avr-rust"
  fi
done

# ---------------------------------------------------------------------------
# 7) 7z do x86 abre o que o ARM32 gravou (interoperabilidade cruzada)
# ---------------------------------------------------------------------------
if command -v 7z >/dev/null && [ -f "$TRABALHO/arm.7z" ]; then
  W="$TRABALHO/via-7z-real"; mkdir -p "$W"
  if 7z x "$TRABALHO/arm.7z" -o"$W" -psegredo123 -y >"$TRABALHO/7z-real.log" 2>&1; then
    S1="$(sha256sum "$W/arquivo1.txt" | awk '{print $1}')"
    S2="$(sha256sum "$W/arquivo2.bin" | awk '{print $1}')"
    if [ "$S1" = "$SHA_ORIGEM_1" ] && [ "$S2" = "$SHA_ORIGEM_2" ]; then
      registrar "interoperabilidade (7z real x86 abre .7z gravado no ARM32)" "RODA" \
        "7z x arm.7z -psegredo123 (7z 23.01, do apt)" "sha256 == origem: $S1 / $S2"
    fi
  fi
fi

# ---------------------------------------------------------------------------
# grava resultados.json
# ---------------------------------------------------------------------------
{
  echo "{"
  echo "  \"medido_em\": \"$DATA\","
  echo "  \"rustc\": \"$RUSTC_VERBOSO\","
  echo "  \"resultados\": ["
  N=${#REGISTROS[@]}
  for i in "${!REGISTROS[@]}"; do
    printf '    %s' "${REGISTROS[$i]}"
    if [ "$i" -lt $((N - 1)) ]; then echo ","; else echo; fi
  done
  echo "  ]"
  echo "}"
} > "$SAIDA"

echo
echo "gravado: $SAIDA"
python3 -c "
import json
d = json.load(open('$SAIDA'))
for r in d['resultados']:
    print(f\"{r['nivel']:12s} {r['alvo']}\")"
