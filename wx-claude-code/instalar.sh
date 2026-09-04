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
#   ./instalar.sh --sim               responde sim a toda pergunta (automacao)
#   ./instalar.sh --corpus /caminho/Help_WL_12k_Json.zip
#
# Falta algum pre-requisito? Ele mostra o comando que resolveria e PERGUNTA
# antes de rodar. Nada e instalado sem voce ver e aprovar; sem terminal
# interativo, ele so diz o comando e para.
set -euo pipefail

RAIZ="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SERIAL=""; CORPUS=""; SO_CONFERIR=0; SEMPRE_SIM=0
while [ $# -gt 0 ]; do
  case "$1" in
    --raiz)     RAIZ="$2"; shift 2 ;;
    --serial)   SERIAL="$2"; shift 2 ;;
    --corpus)   CORPUS="$2"; shift 2 ;;
    --conferir) SO_CONFERIR=1; shift ;;
    --sim|--yes) SEMPRE_SIM=1; shift ;;
    -h|--help)  sed -n '2,15p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) echo "erro: opcao desconhecida $1 (use --help)" >&2; exit 2 ;;
  esac
done

verde() { printf '  \033[32mok\033[0m    %s\n' "$1"; }
falha() { printf '  \033[31mfalha\033[0m %s\n' "$1" >&2; }
aviso() { printf '  \033[33maviso\033[0m %s\n' "$1"; }
passo() { printf '\n\033[1m%s\033[0m\n' "$1"; }
morrer() { falha "$1"; if [ -n "${2:-}" ]; then printf '        %s\n' "$2" >&2; fi; exit 1; }

# Gerenciador de pacotes da maquina, para propor o comando certo. Nao instala
# nada por conta: quem decide e quem le a pergunta.
gerenciador() {
  for g in apt-get dnf yum pacman zypper brew; do
    command -v "$g" >/dev/null && { echo "$g"; return; }
  done
  echo ""
}

comando_para() {  # $1 = ferramenta; imprime o comando que a instalaria, ou nada
  local g; g="$(gerenciador)"
  case "$1:$g" in
    python:apt-get) echo "sudo apt-get update && sudo apt-get install -y python3" ;;
    python:dnf)     echo "sudo dnf install -y python3" ;;
    python:yum)     echo "sudo yum install -y python3" ;;
    python:pacman)  echo "sudo pacman -S --noconfirm python" ;;
    python:zypper)  echo "sudo zypper install -y python3" ;;
    python:brew)    echo "brew install python@3.12" ;;
    claude:*)       command -v npm >/dev/null && echo "npm install -g @anthropic-ai/claude-code" || echo "" ;;
    *) echo "" ;;
  esac
}

perguntar() {  # $1 = pergunta; devolve 0 para sim
  if [ "$SEMPRE_SIM" = 1 ]; then printf '  %s [s/N] s (--sim)\n' "$1"; return 0; fi
  if [ "$SO_CONFERIR" = 1 ]; then printf '  %s [s/N] --conferir: nao pergunta e nao instala\n' "$1"; return 1; fi
  if [ ! -t 0 ]; then printf '  %s [s/N] sem terminal interativo: nao instala\n' "$1"; return 1; fi
  local r; printf '  %s [s/N] ' "$1"; read -r r
  case "$r" in [sSyY]*) return 0 ;; *) return 1 ;; esac
}

instalar_com_aprovacao() {  # $1 = nome legivel, $2 = ferramenta
  local cmd; cmd="$(comando_para "$2")"
  if [ -z "$cmd" ]; then
    aviso "nao sei instalar $1 nesta maquina automaticamente"
    return 1
  fi
  printf '        comando: %s\n' "$cmd"
  if perguntar "instalar $1 agora?"; then
    if bash -c "$cmd"; then verde "$1 instalado"; return 0; fi
    falha "a instalacao de $1 falhou; rode o comando acima a mao"
    return 1
  fi
  if [ "$SO_CONFERIR" = 1 ]; then
    aviso "$1 seria oferecido aqui (--conferir nao instala nada)"
  else
    aviso "$1 nao foi instalado"
  fi
  return 1
}

passo "1. Pre-requisitos"
python_ok() {
  command -v python3 >/dev/null || return 1
  python3 -c 'import sys;raise SystemExit(0 if sys.version_info>=(3,11) else 1)' 2>/dev/null
}
if python_ok; then
  verde "python3 $(python3 -c 'import sys;print("%d.%d"%sys.version_info[:2])')"
else
  if command -v python3 >/dev/null; then
    falha "Python $(python3 -c 'import sys;print("%d.%d"%sys.version_info[:2])') e antigo demais (preciso de 3.11+)"
  else
    falha "python3 nao encontrado"
  fi
  instalar_com_aprovacao "o Python 3.11+" python || true
  python_ok || morrer "sem Python 3.11 ou mais novo nao da para seguir" \
    "instale pelo site python.org ou pelo gerenciador da sua distribuicao, e rode de novo"
  verde "python3 $(python3 -c 'import sys;print("%d.%d"%sys.version_info[:2])')"
fi
if command -v claude >/dev/null; then
  verde "claude $(claude --version 2>/dev/null | head -1)"
else
  aviso "o CLI 'claude' nao esta no PATH"
  instalar_com_aprovacao "o Claude Code" claude || \
    aviso "sem o CLI, o pacote ainda e validado, mas o plugin nao e instalado (veja https://docs.claude.com/en/docs/claude-code)"
fi
if [ ! -f "$RAIZ/.claude-plugin/plugin.json" ]; then
  falha "$RAIZ nao tem .claude-plugin/plugin.json"
  REPO="https://github.com/adrianoboller/adrianoboller.git"
  if command -v git >/dev/null; then
    DESTINO_CLONE="$PWD/adrianoboller"
    printf '        comando: git clone --depth 1 %s %s\n' "$REPO" "$DESTINO_CLONE"
    if perguntar "baixar o plugin do repositorio agora?"; then
      git clone --depth 1 "$REPO" "$DESTINO_CLONE" || morrer "clone falhou" "confira a rede e o acesso ao repositorio"
      RAIZ="$DESTINO_CLONE/wx-claude-code"
      [ -f "$RAIZ/.claude-plugin/plugin.json" ] || morrer "clonei, mas o manifesto nao esta onde eu esperava" "use --raiz apontando para a pasta wx-claude-code"
      verde "plugin baixado em $RAIZ"
    else
      morrer "sem o manifesto nao da para seguir" "use --raiz apontando para a pasta wx-claude-code"
    fi
  else
    morrer "sem o manifesto e sem git para baixar" "use --raiz, ou instale o git e rode de novo"
  fi
fi
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
