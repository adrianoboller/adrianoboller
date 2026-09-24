#!/usr/bin/env bash
# Os portoes de commit, num comando so e num codigo de saida so (pedido 421).
#
# Por que existe: em 23/09/2026 a arvore junta passou por fmt, clippy com zero
# avisos e 2.739 testes verdes -- e subiu com a catraca do mapa da trava
# REPROVADA (`alcancam-fsync-2 25`, teto 24). Os conferidores em Rust rodam
# dentro do `cargo test`; as catracas em Python moram fora e so rodavam se
# alguem lembrasse. Lembrar e memoria, e memoria e o que o portao existe para
# substituir: aqui a suite verde com uma catraca vermelha sai VERMELHO.
#
# Roda os quatro passos sempre, mesmo depois de um vermelho: quem integra
# precisa do quadro inteiro de uma vez, nao de um erro por corrida.
#
#   ./portoes.sh                 # na arvore deste script
#   ./portoes.sh --raiz DIR      # numa arvore exata (ex.: a do commit montada
#                                # por `git archive`), que e onde se prova
#   PHX_CARGO=./cargo-da-frente.sh ./portoes.sh   # o cargo pela vaga das frentes
#
# Sai 0 so se os quatro sairem 0.
set -u

raiz="$(cd "$(dirname "$0")" && pwd)"
while [ $# -gt 0 ]; do
  case "$1" in
    --raiz) raiz="$(cd "$2" && pwd)"; shift 2 ;;
    -h|--help) sed -n '2,20p' "$0"; exit 0 ;;
    *) echo "argumento desconhecido: $1 (use --raiz DIR)" >&2; exit 2 ;;
  esac
done

# `PHX_CARGO` escolhe QUAL cargo chamar -- o do sistema, ou o
# `cargo-da-frente.sh` quando ha frentes compilando em paralelo.
cargo_cmd="${PHX_CARGO:-cargo}"

# Arvore exata sem `.git` (a de `git archive`): a regua dos aprendizados
# confere evidencia por commit e, sem git, reprova como «nao conferivel». A
# arvore saiu do repositorio deste script, entao e nele que o commit se confere.
if ! git -C "$raiz" rev-parse --git-dir >/dev/null 2>&1; then
  repo="$(git -C "$(dirname "$0")" rev-parse --show-toplevel 2>/dev/null)"
  if [ -n "$repo" ] && [ -z "${PHXSQL_GIT_DIR:-}" ]; then
    export PHXSQL_GIT_DIR="$repo"
    echo "(arvore sem git: commits conferidos em $repo)"
  fi
fi
cd "$raiz" || exit 2

nomes=(); codigos=()
passo() {
  local nome="$1"; shift
  echo "== $nome"
  "$@"
  local rc=$?
  nomes+=("$nome"); codigos+=("$rc")
  [ "$rc" = 0 ] || echo "   ^ $nome saiu $rc"
}

passo "fmt"      $cargo_cmd fmt --all --check
# `-D warnings` e o que faz o «zero avisos» virar codigo de saida: sem ele o
# clippy imprime o aviso e sai 0, e o portao voltaria a depender de alguem ler.
passo "clippy"   $cargo_cmd clippy --workspace --all-targets --offline -- -D warnings
passo "suite"    $cargo_cmd test --workspace --offline --no-fail-fast
passo "catracas" python3 bancada/catracas/todas.py

echo
echo "== veredito ($raiz)"
falhou=0
for i in "${!nomes[@]}"; do
  if [ "${codigos[$i]}" = 0 ]; then
    printf '   ok     %s\n' "${nomes[$i]}"
  else
    printf '   FALHOU %s (saiu %s)\n' "${nomes[$i]}" "${codigos[$i]}"
    falhou=1
  fi
done
if [ "$falhou" = 0 ]; then
  echo "VERDE: os quatro portoes passaram."
  exit 0
fi
echo "VERMELHO: suite verde nao basta -- um portao acima reprovou."
exit 1
