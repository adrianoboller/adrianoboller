#!/bin/bash
# Gera o pacote do PhxJev a partir do repositorio -- nunca montado a mao.
#
# Sai um .zip e um .tar.gz com o mesmo conteudo, e o SHA-256 de cada um:
#   phxjev-<versao>/
#     .claude-plugin/marketplace.json   (marketplace «phoenix» apontando para ./phxjev)
#     phxjev/                            (o plugin: skill, comandos, gancho, scripts, bancada)
#     INSTALAR.md
# A versao sai do plugin.json; o pacote so nasce com os testes verdes e o
# plugin validado, e cada arquivo entra pelo `git ls-files` -- o que nao
# esta versionado (cache, registro local, __pycache__) nao vai junto.
set -euo pipefail
AQUI=$(cd "$(dirname "$0")" && pwd)
RAIZ=$(git -C "$AQUI" rev-parse --show-toplevel)
SAIDA=${1:-$RAIZ/dist}
VERSAO=$(python3 -c "import json;print(json.load(open('$AQUI/.claude-plugin/plugin.json'))['version'])")
NOME=phxjev-$VERSAO

python3 "$AQUI/scripts/teste_phxjev.py" >/dev/null 2>&1 || { echo "testes vermelhos: sem pacote" >&2; exit 1; }
if command -v claude >/dev/null 2>&1; then
  claude plugin validate "$AQUI" >/dev/null || { echo "plugin invalido: sem pacote" >&2; exit 1; }
fi

TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT
mkdir -p "$TMP/$NOME/.claude-plugin" "$TMP/$NOME/phxjev"
( cd "$AQUI" && git ls-files -z . ) | while IFS= read -r -d '' f; do
  case "$f" in empacotar.sh|*__pycache__*) continue ;; esac
  mkdir -p "$TMP/$NOME/phxjev/$(dirname "$f")"
  cp -p "$AQUI/$f" "$TMP/$NOME/phxjev/$f"
done
cat >"$TMP/$NOME/.claude-plugin/marketplace.json" <<EOF
{
  "name": "phoenix",
  "owner": { "name": "Adriano Boller" },
  "metadata": { "description": "Plugins do projeto Phoenix." },
  "plugins": [
    { "name": "phxjev", "source": "./phxjev", "version": "$VERSAO",
      "description": "Juiz tipado no molde do Jev: noul/choice/score, limiar em codigo, calibracao medida." }
  ]
}
EOF
cat >"$TMP/$NOME/INSTALAR.md" <<EOF
# PhxJev $VERSAO

Descompacte e, no Claude Code:

    /plugin marketplace add <caminho>/$NOME
    /plugin install phxjev@phoenix

Sem descompactar nada, direto do GitHub:

    /plugin marketplace add adrianoboller/adrianoboller#claude/phxjev-markdown-plugin-vdvios
    /plugin install phxjev@phoenix

So para uma sessao: \`claude --plugin-dir <caminho>/$NOME/phxjev\`

Requer python3. O juiz local (Ollama) e opcional: \`/phxjev-local-instalar\`.
Leia \`phxjev/README.md\`.
EOF

mkdir -p "$SAIDA"
rm -f "$SAIDA/$NOME.zip" "$SAIDA/$NOME.tar.gz"
( cd "$TMP" && zip -qr -X "$SAIDA/$NOME.zip" "$NOME" && tar --sort=name --owner=0 --group=0 --numeric-owner -czf "$SAIDA/$NOME.tar.gz" "$NOME" )
( cd "$SAIDA" && sha256sum "$NOME.zip" "$NOME.tar.gz" > "$NOME.sha256" )
echo "arquivos: $(find "$TMP/$NOME" -type f | wc -l)"
ls -l "$SAIDA/$NOME".* | awk '{print $5, $9}'
cat "$SAIDA/$NOME.sha256"
