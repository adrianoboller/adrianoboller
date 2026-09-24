#!/usr/bin/env bash
# Prova do zelador nos DOIS sentidos, pedidos 317 e 389.
#
#   bancada/zelador/prova-do-zelador.sh [caminho-do-zelador]
#
# Monta uma arvore de mentira num diretorio temporario, copia o zelador para
# dentro dela e confere as decisoes da arvore principal. Com o zelador de
# verdade (o padrao), os oito casos passam; com um defeito reposto, o caso que
# o guarda reprova. O vermelho se refaz trocando o argumento por um mutante:
#
#   389  a linhagem volta a contar  -> sed '/case "$PROPRIOS"/d'
#   317  o corte some               -> sed '/^  cortar_incremental$/d'
#   317  o corte ignora a trava     -> sed 's/if ! flock -n "$trava" true/if false/'
#        (o dado sobrevive: o `rm` roda dentro de um segundo `flock -n`)
#   317  sem as duas camadas        -> o de cima e mais
#                                      sed 's/flock -n "$trava" rm -rf/rm -rf/'
#
# O caso 8 usa o `cargo` DE VERDADE: o que depende do sistema operacional se
# prova contra ele, e «o cargo segura o `.cargo-lock` durante a compilacao» e
# exatamente a premissa em que o corte se apoia. Um projeto de um arquivo,
# sem dependencia, com um `build.rs` que dorme -- e a janela em que o cargo
# esta vivo e segurando a trava.
#
# Hermetico: `PHX_ZELADOR_SO_ARVORE` faz o zelador parar depois da arvore
# principal, porque as secoes de baixo varrem o /tmp e o scratchpad de
# verdade. Nada fora do diretorio temporario e tocado.
set -u
AQUI=$(cd "$(dirname "$0")" && pwd)
ZELADOR=${1:-$AQUI/../../zelador.sh}
[ -r "$ZELADOR" ] || { echo "zelador nao encontrado: $ZELADOR"; exit 2; }

T=$(mktemp -d "${TMPDIR:-/tmp}/phx-prova-zelador.XXXXXX")
VIVOS=()
limpar() {
  local p
  for p in "${VIVOS[@]}"; do kill "$p" 2>/dev/null; done
  wait 2>/dev/null
  rm -rf "$T"
}
trap limpar EXIT
unset CARGO_TARGET_DIR CARGO_BUILD_TARGET_DIR

RAIZ=$T/repo/phxsql
FALHAS=0
passou() { echo "  ok    $1"; }
falhou() { echo "  FALHA $1"; FALHAS=$((FALHAS+1)); }

montar() {
  rm -rf "$T/repo"
  mkdir -p "$RAIZ/bancada" "$RAIZ/target/debug/deps" "$RAIZ/target/debug/incremental/crate-x/s-1"
  cp "$ZELADOR" "$RAIZ/zelador.sh"
  printf '#!/bin/sh\nexit 1\n' >"$RAIZ/bancada/esta-medindo.sh"
  chmod +x "$RAIZ/zelador.sh" "$RAIZ/bancada/esta-medindo.sh"
  printf '[package]\nname = "x"\nversion = "0.0.1"\nedition = "2021"\n' >"$RAIZ/Cargo.toml"
  head -c 2097152 /dev/zero >"$RAIZ/target/debug/incremental/crate-x/s-1/obj"
  head -c 1024 /dev/zero >"$RAIZ/target/debug/deps/binario-de-teste"
  : >"$RAIZ/target/debug/.cargo-lock"
}

# Um processo que NAO e da linhagem do zelador, com `cwd` na arvore: o shell
# de uma frente que pode compilar a qualquer momento.
vizinho() {
  (cd "$RAIZ" && exec sleep 300) &
  VIVOS+=($!)
}

rodar() { # $1 = piso do corte em MiB, resto = argumentos do zelador
  local piso=$1; shift
  (cd "$RAIZ" && PHX_ZELADOR_SO_ARVORE=1 PHX_PISO_INCREMENTAL_MIB=$piso \
    PHX_CACHE_GUARDAS="$T/sem-cache" ./zelador.sh "$@" 2>&1)
}

echo "== prova do zelador: $ZELADOR"

echo "-- 389: a propria linhagem nao conta"
montar
SAIDA=$(rodar 4096 --ver)
if printf '%s' "$SAIDA" | grep -q 'alguem trabalha aqui'; then
  falhou "1. arvore ociosa: o zelador se achou ($(printf '%s' "$SAIDA" | grep -o 'PID [0-9]* ([^)]*)' | head -1))"
else
  printf '%s' "$SAIDA" | grep -q 'reconstroi em minutos' \
    && passou "1. arvore ociosa: libera o target/debug" \
    || falhou "1. arvore ociosa: nem se achou nem liberou"
fi
vizinho
SAIDA=$(rodar 4096 --ver)
printf '%s' "$SAIDA" | grep -q 'alguem trabalha aqui' \
  && passou "2. vizinho vivo com cwd na arvore: recusa o target inteiro" \
  || falhou "2. vizinho vivo com cwd na arvore: apagaria o target de quem trabalha"

INC=$RAIZ/target/debug/incremental
echo "-- 317: o corte cirurgico do incremental, com vizinho vivo"
# A trava se toma NO PROPRIO processo que dorme: com `flock arquivo sleep`, o
# `sleep` e filho do `flock`, matar o `flock` o deixa orfao segurando a
# saida -- e quem le a prova por um pipe espera cinco minutos pelo fim dela.
(exec 9>"$RAIZ/target/debug/.cargo-lock"; flock 9; exec sleep 300) &
VIVOS+=($!)
until ! flock -n "$RAIZ/target/debug/.cargo-lock" true 2>/dev/null; do sleep 0.1; done
SAIDA=$(rodar 999999999)
# O dano e o veredito se medem SEPARADOS: o `rm` roda dentro de um segundo
# `flock -n`, e sem a conferencia de cima o dado sobrevive mesmo assim -- a
# prova diria «apagou» sobre um incremental intacto.
if [ ! -d "$INC" ]; then
  falhou "3. trava do cargo tomada: APAGOU o incremental de quem compila"
elif printf '%s' "$SAIDA" | grep -q 'compilando AGORA'; then
  passou "3. trava do cargo tomada: nao toca o incremental"
else
  falhou "3. trava do cargo tomada: o dado ficou so pela segunda camada; a conferencia nao recusou"
fi
kill "${VIVOS[-1]}" 2>/dev/null; wait "${VIVOS[-1]}" 2>/dev/null

SAIDA=$(rodar 1)
[ -d "$INC" ] && printf '%s' "$SAIDA" | grep -q 'incremental quente' \
  && passou "4. quente e acima do piso: fica" \
  || falhou "4. quente e acima do piso: apagou cache que a proxima compilacao refaz"

rm -f "$RAIZ/target/debug/.cargo-lock"
SAIDA=$(rodar 999999999)
[ -d "$INC" ] && printf '%s' "$SAIDA" | grep -q 'sem trava do cargo' \
  && passou "5. perfil sem trava: fica (flock nao cria trava para conferir)" \
  || falhou "5. perfil sem trava: apagou sem ter o que conferir"
[ -e "$RAIZ/target/debug/.cargo-lock" ] \
  && falhou "5b. a conferencia CRIOU a trava que nao existia"
: >"$RAIZ/target/debug/.cargo-lock"

SAIDA=$(rodar 999999999)
if [ ! -d "$INC" ] && [ -e "$RAIZ/target/debug/deps/binario-de-teste" ] \
   && printf '%s' "$SAIDA" | grep -q 'trava do cargo livre'; then
  passou "6. abaixo do piso e trava livre: corta so o incremental, deps intacto"
else
  falhou "6. abaixo do piso e trava livre: $( [ -d "$INC" ] && echo 'nao cortou' || echo 'levou mais que o incremental')"
fi

# `montar` apaga a arvore, e com ela o `cwd` do vizinho: sem um vizinho novo,
# o zelador cairia no ramo da arvore ociosa e levaria o target/debug inteiro
# -- o caso passaria (ou falharia) pelo ramo errado.
montar
vizinho
touch -d '8 hours ago' "$INC" "$INC/crate-x"
SAIDA=$(rodar 1)
[ ! -d "$INC" ] && printf '%s' "$SAIDA" | grep -q 'frio: sem compilar' \
  && passou "7. frio (8 h sem compilar), acima do piso: corta" \
  || falhou "7. frio, acima do piso: nao cortou"

echo "-- 317: contra o cargo DE VERDADE"
if ! command -v cargo >/dev/null; then
  falhou "8. cargo ausente: a premissa da trava nao foi provada"
else
  montar
  vizinho
  rm -f "$RAIZ/target/debug/.cargo-lock"
  mkdir -p "$RAIZ/src"
  : >"$RAIZ/src/lib.rs"
  printf 'fn main() { std::thread::sleep(std::time::Duration::from_secs(20)); }\n' >"$RAIZ/build.rs"
  (cd "$RAIZ" && exec cargo build --offline -q >/dev/null 2>&1) &
  CARGO=$!
  VIVOS+=($CARGO)
  n=0
  until [ -e "$RAIZ/target/debug/.cargo-lock" ] \
        && ! flock -n "$RAIZ/target/debug/.cargo-lock" true 2>/dev/null; do
    sleep 0.2; n=$((n+1)); [ $n -gt 150 ] && break
  done
  SAIDA=$(rodar 999999999)
  if [ ! -d "$INC" ]; then
    falhou "8. cargo compilando de verdade: APAGOU o incremental no meio da compilacao"
  elif printf '%s' "$SAIDA" | grep -q 'compilando AGORA'; then
    passou "8. cargo compilando de verdade: a trava esta tomada e o incremental fica"
  else
    falhou "8. cargo compilando de verdade: ficou so pela segunda camada ($(printf '%s' "$SAIDA" | grep -c incremental) linha(s) do corte)"
  fi
  wait "$CARGO"
  SAIDA=$(rodar 999999999)
  [ ! -d "$INC" ] && printf '%s' "$SAIDA" | grep -q 'trava do cargo livre' \
    && passou "9. cargo terminou: a mesma trava fica livre e o corte acontece" \
    || falhou "9. cargo terminou: nao cortou ($(printf '%s' "$SAIDA" | grep incremental | head -1))"
fi

echo "== $([ $FALHAS -eq 0 ] && echo 'VERDE' || echo "VERMELHO: $FALHAS falha(s)")"
[ $FALHAS -eq 0 ]
