#!/usr/bin/env bash
# Gera e assina o MANIFESTO de atualizacao (`phxvpn atualizar` le este
# arquivo) a partir dos binarios que o `empacotar.sh` produz. Como o
# `empacotar.sh`, nunca monta nada a mao: chama o `cargo build --release`
# direto e assina com `phxvpn atualizar-assinar`.
#
#   ./publicar-atualizacao.sh --versao X.Y.Z --base-url https://onde/os/binarios/vao/ficar
#                              [--saida pacotes/manifesto.json]
#                              [--chave-privada ARQUIVO]   (senao: PHXVPN_CHAVE_PRIVADA_ATUALIZACAO)
#
# A chave privada NUNCA e gravada por este script -- so passa pelo caminho de
# arquivo (que o chamador escolhe onde guardar, fora do repositorio) ou pela
# variavel de ambiente, do mesmo jeito que `phxvpn atualizar-assinar` ja
# exige. Windows ARM64 nao entra: sem cadeia de link para
# aarch64-pc-windows-gnullvm neste ambiente (ver docs/PHXVPN.md).
#
# Saida em $SAIDA: manifesto.json, phxvpn-linux-x86_64, phxvpn-windows-x86_64.exe
# -- os tres vao para o mesmo lugar que $BASE_URL aponta.
set -euo pipefail
AQUI=$(cd "$(dirname "$0")" && pwd)

VERSAO=""
BASE_URL=""
SAIDA_DIR="$AQUI/pacotes"
CHAVE_PRIVADA_ARQUIVO=""

while [ $# -gt 0 ]; do
    case "$1" in
        --versao) VERSAO="$2"; shift 2 ;;
        --base-url) BASE_URL="${2%/}"; shift 2 ;;
        --saida) SAIDA_DIR="$2"; shift 2 ;;
        --chave-privada) CHAVE_PRIVADA_ARQUIVO="$2"; shift 2 ;;
        *) echo "opcao desconhecida: $1" >&2; exit 1 ;;
    esac
done
[ -n "$VERSAO" ] || { echo "informe --versao X.Y.Z" >&2; exit 1; }
[ -n "$BASE_URL" ] || { echo "informe --base-url (onde os binarios vao ficar hospedados)" >&2; exit 1; }
if [ -z "$CHAVE_PRIVADA_ARQUIVO" ] && [ -z "${PHXVPN_CHAVE_PRIVADA_ATUALIZACAO:-}" ]; then
    echo "informe --chave-privada ARQUIVO ou PHXVPN_CHAVE_PRIVADA_ATUALIZACAO" >&2
    exit 1
fi

mkdir -p "$SAIDA_DIR"
SAIDA_DIR=$(cd "$SAIDA_DIR" && pwd)

echo "== compilando $VERSAO (Linux e Windows x86_64, release)"
(cd "$AQUI" && cargo build --release -q && cargo build --release -q --target x86_64-pc-windows-gnu)

LIN_BIN="$AQUI/target/release/phxvpn"
WIN_BIN="$AQUI/target/x86_64-pc-windows-gnu/release/phxvpn.exe"
[ -f "$LIN_BIN" ] || { echo "faltou compilar $LIN_BIN" >&2; exit 1; }
[ -f "$WIN_BIN" ] || { echo "faltou compilar $WIN_BIN" >&2; exit 1; }

cp "$LIN_BIN" "$SAIDA_DIR/phxvpn-linux-x86_64"
cp "$WIN_BIN" "$SAIDA_DIR/phxvpn-windows-x86_64.exe"

SHA_LIN=$(sha256sum "$SAIDA_DIR/phxvpn-linux-x86_64" | cut -d' ' -f1)
SHA_WIN=$(sha256sum "$SAIDA_DIR/phxvpn-windows-x86_64.exe" | cut -d' ' -f1)

MANIFESTO="$SAIDA_DIR/manifesto.json"
CHAVE_ARGS=()
if [ -n "$CHAVE_PRIVADA_ARQUIVO" ]; then
    CHAVE_ARGS=(--chave-privada "$CHAVE_PRIVADA_ARQUIVO")
fi

echo "== assinando manifesto"
(cd "$AQUI" && cargo run --release -q --bin phxvpn -- atualizar-assinar \
    --versao "$VERSAO" \
    --saida "$MANIFESTO" \
    "${CHAVE_ARGS[@]}" \
    --alvo "linux-x86_64=$BASE_URL/phxvpn-linux-x86_64,$SHA_LIN" \
    --alvo "windows-x86_64=$BASE_URL/phxvpn-windows-x86_64.exe,$SHA_WIN")

echo "== pronto em $SAIDA_DIR"
ls -l "$SAIDA_DIR/manifesto.json" "$SAIDA_DIR/phxvpn-linux-x86_64" "$SAIDA_DIR/phxvpn-windows-x86_64.exe"
echo "== suba os tres arquivos para $BASE_URL/ (mesmos nomes) e confira: CHAVE_PUBLICA_PADRAO em src/atualizar.rs precisa ser o par da chave privada usada aqui"
