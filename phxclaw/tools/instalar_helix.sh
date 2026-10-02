#!/usr/bin/env bash
# Compila o editor Helix (Rust, MPL-2.0) do fonte e o instala em /opt/helix, para o IDE do
# PhxClaw rodar o `hx` dentro do motor de terminal (crates/phxclaw-terminal).
#
# O Helix entra como PROCESSO SEPARADO e SEM MODIFICACAO: e o que deixa a MPL-2.0 dele longe
# do nosso Apache-2.0 (nenhum arquivo dele vira nosso) e o que permite trocar a versao sem
# tocar no PhxClaw. Por isso compila-se do fonte da tag, sem patch.
#
# Por que do fonte e nao o binario da release: download de release do GitHub da 403 neste
# ambiente, e git funciona. A tag e o commit ficam fixados abaixo e conferidos depois do
# clone -- tag de git pode ser movida, commit nao.
#
# Uso: tools/instalar_helix.sh            (DESTINO, FONTE, CARGO_TARGET_DIR, PRECISA_MIB
#                                          podem vir do ambiente)
set -euo pipefail

TAG=25.07.1
# O COMMIT da tag, e nao o objeto da tag: 25.07.1 e tag ANOTADA, e o objeto dela
# (ac94841...) e outro numero. Medido em 01/10: comparado com o HEAD, o objeto da tag
# parava toda instalacao; o commit e o que o `git ls-remote` da como 25.07.1^{}.
COMMIT=a05c151bb6e8e9c65ec390b0ae2afe7a5efd619b
DESTINO=${DESTINO:-/opt/helix}
FONTE=${FONTE:-/var/tmp/helix-src}
export CARGO_TARGET_DIR=${CARGO_TARGET_DIR:-/var/tmp/helix-target}
export CARGO_INCREMENTAL=0
# Sem informacao de depuracao: o hx instalado nao e depurado aqui, e ela dobra o target.
export CARGO_PROFILE_RELEASE_DEBUG=0
# O repositorio do Helix fixa a toolchain dele no rust-toolchain.toml; obedecer baixaria
# outra toolchain inteira so para isto. A do PhxClaw atende o MSRV dele.
export RUSTUP_TOOLCHAIN=${RUSTUP_TOOLCHAIN:-1.98.1}
# So as gramaticas que o IDE usa: as ~250 do languages.toml custam centenas de MB de fonte
# baixada e de .so compilado.
GRAMATICAS=(rust python toml json markdown sql html css javascript)
# Piso de disco livre para comecar. E ESTIMATIVA, nao medida (a compilacao ainda nao rodou
# nesta maquina): target de release do helix-term sem debug, fonte e gramaticas. Quando a
# primeira corrida terminar, o `du` do fim do script da o numero para trocar este.
PRECISA_MIB=${PRECISA_MIB:-3072}

livre_mib() { df -Pm "$1" | awk 'NR==2{print $4}'; }

# O languages.toml do IDE: o phxclaw-snippet-ls como servidor adicional (os servidores
# padrao de cada linguagem continuam), e o `gdb -i dap` como depurador do Rust -- o
# lldb-dap que o Helix pede por padrao nao existe nesta maquina, o gdb 15 existe e fala
# DAP. Python fica sem depurador no editor ate haver debugpy no hospedeiro.
escrever_languages_toml() {
  cat > "$1" <<EOF_TOML
# Gerado por tools/instalar_helix.sh (PhxClaw). Mescla-se ao languages.toml padrao do Helix.
[language-server.phxclaw-snippet-ls]
command = "$DESTINO/phxclaw-snippet-ls"

[[language]]
name = "rust"
language-servers = ["rust-analyzer", "phxclaw-snippet-ls"]

[language.debugger]
name = "gdb"
transport = "stdio"
command = "gdb"
args = ["-q", "-i", "dap"]

[[language.debugger.templates]]
name = "binary"
request = "launch"
completion = [ { name = "binary", completion = "filename" } ]
args = { program = "{0}" }

[[language]]
name = "python"
language-servers = ["pyright", "phxclaw-snippet-ls"]

[[language]]
name = "html"
language-servers = ["vscode-html-language-server", "phxclaw-snippet-ls"]

[[language]]
name = "css"
language-servers = ["vscode-css-language-server", "phxclaw-snippet-ls"]

[[language]]
name = "javascript"
language-servers = ["typescript-language-server", "phxclaw-snippet-ls"]

[[language]]
name = "toml"
language-servers = ["taplo", "phxclaw-snippet-ls"]

[[language]]
name = "markdown"
language-servers = ["marksman", "phxclaw-snippet-ls"]

[[language]]
name = "sql"
language-servers = ["phxclaw-snippet-ls"]

[[language]]
name = "json"
language-servers = ["vscode-json-language-server", "phxclaw-snippet-ls"]
EOF_TOML
}
mkdir -p "$(dirname "$FONTE")"
echo "== disco antes de comecar"
df -h "$(dirname "$FONTE")" | tail -1
du -sh "$FONTE" "$CARGO_TARGET_DIR" 2>/dev/null || true
LIVRE=$(livre_mib "$(dirname "$FONTE")")
if (( LIVRE < PRECISA_MIB )); then
  echo "PARADO: ${LIVRE} MiB livres, o piso e ${PRECISA_MIB} MiB. Nada foi baixado nem compilado." >&2
  exit 3
fi

mkdir -p "$CARGO_TARGET_DIR"
if [[ ! -d "$FONTE/.git" ]]; then
  git clone --depth 1 --branch "$TAG" https://github.com/helix-editor/helix "$FONTE"
fi
ACHADO=$(git -C "$FONTE" rev-parse HEAD)
TAG_APONTA=$(git -C "$FONTE" rev-parse "$TAG^{commit}" 2>/dev/null || echo "?")
if [[ "$TAG_APONTA" != "$COMMIT" ]]; then
  echo "PARADO: a tag $TAG aponta para $TAG_APONTA, e o fixado e $COMMIT (tag movida?)" >&2
  exit 4
fi
if [[ "$ACHADO" != "$COMMIT" ]]; then
  echo "PARADO: $FONTE esta em $ACHADO, a tag $TAG fixada e $COMMIT" >&2
  exit 4
fi

# Gramaticas fora do cargo build: o build.rs do helix-term compilaria todas as do
# languages.toml. Depois, o proprio hx baixa e compila so as escolhidas, lendo um
# languages.toml de usuario num XDG_CONFIG_HOME temporario (o ~/.config de quem roda nao
# e tocado).
CFG=$(mktemp -d)
trap 'rm -rf "$CFG"' EXIT
mkdir -p "$CFG/helix"
LISTA=$(printf '"%s", ' "${GRAMATICAS[@]}")
printf 'use-grammars = { only = [ %s ] }\n' "${LISTA%, }" > "$CFG/helix/languages.toml"

(cd "$FONTE" && HELIX_DISABLE_AUTO_GRAMMAR_BUILD=1 cargo build --release --locked -p helix-term)
HX="$CARGO_TARGET_DIR/release/hx"
XDG_CONFIG_HOME="$CFG" HELIX_RUNTIME="$FONTE/runtime" "$HX" --grammar fetch
XDG_CONFIG_HOME="$CFG" HELIX_RUNTIME="$FONTE/runtime" "$HX" --grammar build
# O fonte das gramaticas so serve para compilar; instalado, o hx carrega o .so.
rm -rf "$FONTE/runtime/grammars/sources"

mkdir -p "$DESTINO"
install -m 0755 "$HX" "$DESTINO/hx"
rm -rf "$DESTINO/runtime"
cp -r "$FONTE/runtime" "$DESTINO/runtime"
cp "$FONTE/LICENSE" "$DESTINO/LICENSE" 2>/dev/null || true
printf '%s %s\n' "$TAG" "$COMMIT" > "$DESTINO/VERSAO"

# O servidor de snippets e Emmet do PhxClaw (crates/phxclaw-snippet-ls): o Helix so expande
# snippet que vem por LSP, e o IDE nao arrasta node. Entra no languages.toml como servidor
# adicional de cada linguagem, ao lado do rust-analyzer e do pyright.
PHXCLAW_RAIZ=$(cd "$(dirname "$0")/.." && pwd)
(cd "$PHXCLAW_RAIZ" && CARGO_INCREMENTAL=0 cargo build --release -q -p phxclaw-snippet-ls)
install -m 0755 "$PHXCLAW_RAIZ/target/release/phxclaw-snippet-ls" "$DESTINO/phxclaw-snippet-ls"
escrever_languages_toml "$DESTINO/languages.toml"
# O hx le `$XDG_CONFIG_HOME/helix/languages.toml` (nao ha variavel para apontar outro,
# e o `.helix/` do projeto so vale por projeto). O do usuario, se ja existe, fica: e
# dele; o modelo esta em $DESTINO/languages.toml para mesclar a mao.
CONF="${XDG_CONFIG_HOME:-$HOME/.config}/helix"
mkdir -p "$CONF"
if [[ -e "$CONF/languages.toml" ]]; then
  echo "AVISO: $CONF/languages.toml ja existe e NAO foi tocado; mescle $DESTINO/languages.toml nele"
else
  cp "$DESTINO/languages.toml" "$CONF/languages.toml"
fi

echo "== instalado"
"$DESTINO/hx" --version
ls "$DESTINO/runtime/grammars"
du -sh "$DESTINO" "$CARGO_TARGET_DIR"
df -h "$(dirname "$FONTE")" | tail -1
