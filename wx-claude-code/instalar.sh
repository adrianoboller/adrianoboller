#!/usr/bin/env bash
# Instalador do WX Claude Code (Linux e macOS).
#
# Faz o caminho inteiro e para no primeiro problema dizendo qual e: confere os
# pre-requisitos, poe o corpus no lugar, valida o pacote, instala o plugin no
# Claude Code e, se voce passar um serial, ativa a licenca.
#
# Nao instala nada escondido e nao mexe em nada fora de ~/.claude e
# ~/.wx-claude-code.
#
#   ./instalar.sh                     instala do jeito normal
#   ./instalar.sh --serial "WX2.…"    instala e ativa a licenca
#   ./instalar.sh --conferir          so confere, nao muda nada
#   ./instalar.sh --corpus /caminho/Help_WL_12k_Json.zip
set -euo pipefail

RAIZ="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SERIAL=""; CORPUS=""; SO_CONFERIR=0
while [ $# -gt 0 ]; do
  case "$1" in
    --raiz)     RAIZ="$2"; shift 2 ;;
    --serial)   SERIAL="$2"; shift 2 ;;
    --corpus)   CORPUS="$2"; shift 2 ;;
    --conferir) SO_CONFERIR=1; shift ;;
    -h|--help)  sed -n '2,15p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) echo "erro: opcao desconhecida $1 (use --help)" >&2; exit 2 ;;
  esac
done

verde() { printf '  \033[32mok\033[0m    %s\n' "$1"; }
falha() { printf '  \033[31mfalha\033[0m %s\n' "$1" >&2; }
aviso() { printf '  \033[33maviso\033[0m %s\n' "$1"; }
passo() { printf '\n\033[1m%s\033[0m\n' "$1"; }
morrer() { falha "$1"; if [ -n "${2:-}" ]; then printf '        %s\n' "$2" >&2; fi; exit 1; }

passo "1. Pre-requisitos"
command -v python3 >/dev/null || morrer "python3 nao encontrado" "instale o Python 3.11 ou mais novo"
PYV="$(python3 -c 'import sys;print("%d.%d"%sys.version_info[:2])')"
python3 -c 'import sys;raise SystemExit(0 if sys.version_info>=(3,11) else 1)' \
  || morrer "Python $PYV e antigo demais" "o plugin precisa de 3.11 ou mais novo"
verde "python3 $PYV"
if command -v claude >/dev/null; then
  verde "claude $(claude --version 2>/dev/null | head -1)"
else
  aviso "o CLI 'claude' nao esta no PATH; a instalacao do plugin sera pulada"
fi
[ -f "$RAIZ/.claude-plugin/plugin.json" ] || morrer "$RAIZ nao parece a pasta do plugin" "falta .claude-plugin/plugin.json; use --raiz"
VERSAO="$(python3 -c "import json;print(json.load(open('$RAIZ/.claude-plugin/plugin.json'))['version'])")"
verde "pacote encontrado: WX Claude Code $VERSAO"

passo "2. Corpus do Help WLanguage"
DESTINO_CORPUS="$RAIZ/skills/conversao-wx/resources/Help_WL_12k_Json.zip"
if [ -n "$CORPUS" ]; then
  [ -f "$CORPUS" ] || morrer "corpus nao encontrado em $CORPUS"
  if [ "$SO_CONFERIR" = 0 ]; then
    mkdir -p "$(dirname "$DESTINO_CORPUS")"
    cp "$CORPUS" "$DESTINO_CORPUS"
    verde "corpus copiado de $CORPUS"
  else
    verde "corpus seria copiado de $CORPUS"
  fi
fi
if [ -f "$DESTINO_CORPUS" ]; then
  MB="$(python3 -c "import os;print(os.path.getsize('$DESTINO_CORPUS')//1048576)")"
  verde "corpus no lugar (${MB} MB)"
else
  aviso "corpus ausente: o G0 fica DEGRADED e a semantica WLanguage some"
  aviso "use --corpus /caminho/Help_WL_12k_Json.zip (parte 2 do pacote)"
fi

passo "3. Conferencia do pacote"
# Arquivo temporario proprio, apagado ao sair: --conferir nao deve deixar rastro
# nem em /tmp (achado pela prova real, que reparou no /tmp/wx-validacao.json fixo).
VALIDACAO="$(mktemp -t wx-validacao.XXXXXX)"
trap 'rm -f "$VALIDACAO"' EXIT
if python3 "$RAIZ/skills/conversao-wx/scripts/validate_plugin_bundle.py" "$RAIZ" >"$VALIDACAO" 2>&1; then
  python3 -c "
import json, sys
d = json.load(open(sys.argv[1]))
print(f\"  ok    valido: {d['skills']} skills, {d['agents']} agentes, {len(d['errors'])} erros\")" "$VALIDACAO"
else
  cat "$VALIDACAO" >&2
  morrer "o pacote nao passou na validacao" "veja os erros acima; nao instale assim"
fi
if command -v claude >/dev/null; then
  if claude plugin validate "$RAIZ" >/dev/null 2>&1; then
    verde "manifesto aceito pelo claude"
  else
    aviso "claude plugin validate reclamou; siga com cuidado"
  fi
fi

passo "4. Instalacao no Claude Code"
if [ "$SO_CONFERIR" = 1 ]; then
  aviso "--conferir: nada foi instalado"
elif command -v claude >/dev/null; then
  PAI="$(dirname "$RAIZ")"
  if [ -f "$PAI/.claude-plugin/marketplace.json" ]; then
    claude plugin marketplace add "$PAI" >/dev/null 2>&1 || true
    if claude plugin install wx-claude-code@wx-claude-code >/dev/null 2>&1; then
      verde "plugin instalado do marketplace local"
    else
      aviso "instalacao pelo marketplace falhou; use: claude --plugin-dir \"$RAIZ\""
    fi
  else
    aviso "marketplace.json nao esta ao lado; use: claude --plugin-dir \"$RAIZ\""
  fi
else
  aviso "sem o CLI claude; depois rode: claude plugin marketplace add <pasta-pai> && claude plugin install wx-claude-code@wx-claude-code"
fi

passo "5. Licenca"
LIC="$RAIZ/skills/conversao-wx/scripts/licenca.py"
if [ -n "$SERIAL" ] && [ "$SO_CONFERIR" = 0 ]; then
  python3 "$LIC" instalar "$SERIAL" >/dev/null \
    || morrer "serial recusado" "confira se copiou inteiro e se e desta maquina (licenca.py maquina)"
fi
if python3 "$LIC" verificar 2>/dev/null | grep -q '^valida'; then
  verde "$(python3 "$LIC" verificar)"
else
  aviso "sem licenca valida: os hooks vao recusar os scripts do plugin"
  aviso "mande ao fornecedor a saida de: python3 $LIC maquina"
fi

passo "Pronto"
cat <<FIM
  Comece por aqui, dentro da pasta do projeto de destino:

    /wx-claude-code:questionario     o questionario inteiro (bloco 0, A a M)
    /wx-claude-code:comandos         o indice dos comandos e das perguntas

  Manual: $RAIZ/MANUAL.md e docs/manual-de-uso.pdf
  Ativacao por serial: $RAIZ/licenca/ATIVACAO.md
FIM
