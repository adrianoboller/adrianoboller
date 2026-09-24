#!/bin/bash
# Instala e sobe o juiz local do PhxJev onde o agente estiver rodando.
# So roda depois de o usuario confirmar (o comando /phxjev-local-instalar
# pergunta antes): instalar programa na maquina de alguem nao se faz calado.
#
#   nuvem do Claude Code  -> compila do fonte (a release nao passa pelo proxy)
#   Linux / macOS         -> instalador oficial de ollama.com
#   Windows (Git Bash)    -> winget
# Uso: instalar-local.sh [modelo]   (padrao: o do .phxjev/config.json, ou qwen2.5:3b)
set -euo pipefail
AQUI=$(cd "$(dirname "$0")" && pwd)
MODELO=${1:-$(python3 -c "import sys; sys.path.insert(0, '$AQUI'); import phxjev; print(phxjev.ler_config()['modelo'])")}
HOST=${OLLAMA_HOST:-127.0.0.1:11434}

no_ar() { curl -fsS "http://$HOST/api/version" >/dev/null 2>&1; }

if no_ar; then
  echo "ollama ja esta no ar em $HOST"
elif [ -n "${CLAUDE_CODE_REMOTE:-}" ]; then
  echo "nuvem do Claude Code: compilando do fonte (uns 4 min) ..."
  MODELOS="$MODELO" bash "$AQUI/../bancada/montar-ollama.sh"
  exit 0
else
  if ! command -v ollama >/dev/null 2>&1; then
    case "$(uname -s)" in
      Linux|Darwin) curl -fsSL https://ollama.com/install.sh | sh ;;
      MINGW*|MSYS*|CYGWIN*) winget install --id Ollama.Ollama -e --accept-source-agreements --accept-package-agreements ;;
      *) echo "sistema $(uname -s) sem instalador conhecido: instale de https://ollama.com/download" >&2; exit 1 ;;
    esac
  fi
  no_ar || { nohup ollama serve >/dev/null 2>&1 & for _ in $(seq 50); do no_ar && break; sleep 0.2; done; }
fi
no_ar || { echo "ollama nao subiu em $HOST" >&2; exit 1; }
# Pela API, e nao pelo binario: o Ollama no ar pode nao estar no PATH (o
# compilado na nuvem mora fora dele), e o `ollama pull` falhava ali.
curl -fsS "http://$HOST/api/pull" -d "{\"model\":\"$MODELO\",\"stream\":false}" | grep -q '"success"' \
  || { echo "falhou baixar $MODELO" >&2; exit 1; }
echo "modelo pronto: $MODELO em $HOST"
echo "lembrete: o juiz so troca se o modelo passar na bancada -- python3 $AQUI/phxjev.py juiz"
