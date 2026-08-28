#!/usr/bin/env bash
# Monta os pacotes de download: Linux e Windows, fontes e compilado, com o
# manual junto.
#
# Existe por um motivo simples: os pacotes das rodadas anteriores foram feitos
# a mao. Pacote feito a mao e pacote que ninguem consegue refazer igual -- e a
# primeira coisa que alguem pede quando o binario da problema e "como voce
# gerou isso?".
#
#   ./empacotar.sh            monta os dois pacotes em pacotes/
#   ./empacotar.sh linux      so o de Linux
#   ./empacotar.sh windows    so o de Windows
#
# Requer o alvo x86_64-pc-windows-gnu instalado para a parte de Windows:
#   rustup target add x86_64-pc-windows-gnu

set -euo pipefail
cd "$(dirname "$0")"

VERSAO=$(grep -m1 '^version' Cargo.toml | cut -d'"' -f2)
SAIDA=pacotes
QUAL=${1:-tudo}

# A interface e os exemplos entram por include_str!, entao o pacote e so os
# dois binarios mais a documentacao. Nada de arquivo solto para se perder.
DOCS=(MANUAL.txt README.md CHANGELOG.md)

mkdir -p "$SAIDA"

monta() {
  local alvo=$1 rotulo=$2 sufixo=$3
  local nome="phxsql-$VERSAO-$rotulo"
  local dir="$SAIDA/$nome"

  echo "== $rotulo ($alvo)"
  cargo build --release --offline --workspace --target "$alvo"

  rm -rf "$dir"; mkdir -p "$dir"
  cp "target/$alvo/release/phxsqld$sufixo" "$dir/"
  cp "target/$alvo/release/phxsql$sufixo"  "$dir/"
  cp "${DOCS[@]}" "$dir/"

  ( cd "$SAIDA" && zip -qr "$nome.zip" "$nome" )
  rm -rf "$dir"
  echo "   $SAIDA/$nome.zip"
}

fontes() {
  local nome="phxsql-$VERSAO-fontes"
  echo "== fontes"
  # git archive respeita o .gitignore de graca: os 2,4 GB da bancada ficam
  # de fora sem ninguem precisar lembrar.
  git archive --format=zip --prefix="$nome/" -o "$SAIDA/$nome.zip" HEAD
  echo "   $SAIDA/$nome.zip"
}

case "$QUAL" in
  linux)   monta x86_64-unknown-linux-gnu linux "" ;;
  windows) monta x86_64-pc-windows-gnu windows .exe ;;
  tudo)
    monta x86_64-unknown-linux-gnu linux ""
    monta x86_64-pc-windows-gnu windows .exe
    fontes
    ;;
  *) echo "uso: $0 [linux|windows|tudo]" >&2; exit 2 ;;
esac

echo
ls -lh "$SAIDA"/*.zip
