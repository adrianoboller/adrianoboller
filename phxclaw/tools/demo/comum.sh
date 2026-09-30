# Funcoes das cenas de terminal. Cada comando aparece "digitado" e depois roda DE VERDADE:
# nada aqui e saida gravada ou simulada.
cd "$(dirname "${BASH_SOURCE[0]}")/../.."
AM=$'\e[1;33m'; CI=$'\e[1;36m'; VE=$'\e[1;32m'; CZ=$'\e[0;37m'; NE=$'\e[1m'; RS=$'\e[0m'
titulo() { clear; printf '%s\n%s\n\n' "${AM}▌ $1${RS}" "${CZ}  $2${RS}"; sleep 1.5; }
digita() {
  printf '%s' "${CI}phxclaw \$ ${RS}"
  local t="$1" i
  for ((i = 0; i < ${#t}; i++)); do printf '%s' "${t:i:1}"; sleep 0.025; done
  printf '\n'; sleep 0.4
}
roda() { digita "$1"; eval "$1"; printf '\n'; sleep "${2:-2}"; }
nota() { printf '%s\n\n' "${VE}✔ $1${RS}"; sleep "${2:-2.5}"; }
