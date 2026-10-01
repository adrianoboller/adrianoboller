#!/usr/bin/env bash
# PORTOES: todos os passos que reprovam um commit, num comando so e num
# codigo de saida so. Generalizado do `portoes.sh` do PhxSql.
#
# Por que existe: uma arvore passou por fmt, clippy com zero avisos e 2.739
# testes verdes -- e subiu com uma catraca REPROVADA, porque as catracas em
# script moravam fora da suite e so rodavam se alguem lembrasse. Aqui a suite
# verde com uma catraca vermelha sai VERMELHO. Roda todos os passos mesmo
# depois de um vermelho: quem integra precisa do quadro inteiro de uma vez.
#
# USO
#   portoes.sh                    # na arvore atual (diretorio do projeto)
#   portoes.sh --raiz DIR         # numa arvore exata (ex.: `git archive` do commit)
#   portoes.sh --passos ARQ       # lista de passos de outro arquivo
#
# OS PASSOS saem de um arquivo `portoes.passos` na raiz (ou de --passos), uma
# linha por passo, no formato `nome | comando`. Linha vazia e `#` sao
# ignoradas. Sem o arquivo, vale o padrao de um projeto Rust:
#
#   fmt      | cargo fmt --all --check
#   clippy   | cargo clippy --workspace --all-targets -- -D warnings
#   suite    | cargo test --workspace --no-fail-fast
#
# Dica: o linter tem de transformar aviso em codigo de saida (`-D warnings`
# no clippy, `--max-warnings 0` no eslint...). Sem isso ele imprime o aviso e
# sai 0, e o portao volta a depender de alguem ler.
#
# Arvore sem `.git` (a de `git archive`): exporta KIT_GIT_DIR apontando para o
# repositorio deste script, para os passos que conferem commit por hash.
#
# Sai 0 so se todos os passos sairem 0.
set -u

raiz="$PWD"
passos_arq=""
while [ $# -gt 0 ]; do
  case "$1" in
    --raiz) raiz="$(cd "$2" && pwd)"; shift 2 ;;
    --passos) passos_arq="$(cd "$(dirname "$2")" && pwd)/$(basename "$2")"; shift 2 ;;
    -h|--help) sed -n '2,32p' "$0"; exit 0 ;;
    *) echo "argumento desconhecido: $1 (use --raiz DIR ou --passos ARQ)" >&2; exit 2 ;;
  esac
done

if ! git -C "$raiz" rev-parse --git-dir >/dev/null 2>&1; then
  repo="$(git -C "$(dirname "$0")" rev-parse --show-toplevel 2>/dev/null)"
  if [ -n "$repo" ] && [ -z "${KIT_GIT_DIR:-}" ]; then
    export KIT_GIT_DIR="$repo"
    echo "(arvore sem git: commits conferidos em $repo)"
  fi
fi
cd "$raiz" || exit 2
[ -z "$passos_arq" ] && [ -f "$raiz/portoes.passos" ] && passos_arq="$raiz/portoes.passos"

nomes=(); cmds=()
if [ -n "$passos_arq" ]; then
  while IFS= read -r linha || [ -n "$linha" ]; do
    case "$linha" in ''|'#'*) continue ;; esac
    nomes+=("$(printf '%s' "${linha%%|*}" | xargs)")
    cmds+=("$(printf '%s' "${linha#*|}" | sed 's/^ *//')")
  done < "$passos_arq"
  echo "(passos de $passos_arq)"
else
  nomes=(fmt clippy suite)
  cmds=("cargo fmt --all --check"
        "cargo clippy --workspace --all-targets -- -D warnings"
        "cargo test --workspace --no-fail-fast")
  echo "(passos padrao de projeto Rust: nao ha portoes.passos em $raiz)"
fi
[ "${#nomes[@]}" -gt 0 ] || { echo "nenhum passo definido" >&2; exit 2; }

codigos=()
for i in "${!nomes[@]}"; do
  echo "== ${nomes[$i]}"
  bash -c "${cmds[$i]}"
  rc=$?
  codigos+=("$rc")
  [ "$rc" = 0 ] || echo "   ^ ${nomes[$i]} saiu $rc"
done

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
  echo "VERDE: os ${#nomes[@]} portoes passaram."
  exit 0
fi
echo "VERMELHO: suite verde nao basta -- um portao acima reprovou."
exit 1
