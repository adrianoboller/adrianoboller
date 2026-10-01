#!/bin/sh
# Salva o trabalho de TODA copia de trabalho de frente como objeto do git,
# numa referencia protegida (refs/salvas/<frente>/<carimbo>), SEM tocar no
# indice nem nos arquivos do agente: usa um indice temporario proprio.
# Ordem do dono, 01/10/2026: «isso nao pode acontecer» -- trabalho de frente
# nunca mora so numa pasta que um comando pode apagar.
cd "$(git -C "$(dirname "$0")" rev-parse --show-toplevel)" || exit 1
CARIMBO=$(date +%Y%m%d-%H%M%S)
for d in .claude/worktrees/*/; do
  d=${d%/}; [ -e "$d/.git" ] || continue; nome=${d##*/}
  [ -n "$(git -C "$d" status --porcelain 2>/dev/null)" ] || { echo "limpa: $nome"; continue; }
  IDX=$(mktemp); cp "$(git -C "$d" rev-parse --git-path index)" "$IDX" 2>/dev/null
  arvore=$(GIT_INDEX_FILE=$IDX git -C "$d" add -A . >/dev/null 2>&1 && GIT_INDEX_FILE=$IDX git -C "$d" write-tree)
  rm -f "$IDX"
  [ -n "$arvore" ] || { echo "FALHOU ao salvar: $nome"; continue; }
  pai=$(git -C "$d" rev-parse HEAD)
  ultima=$(git for-each-ref --sort=-refname --count=1 --format='%(objectname)' "refs/salvas/$nome/")
  if [ -n "$ultima" ] && [ "$(git rev-parse "$ultima^{tree}")" = "$arvore" ]; then echo "sem mudanca: $nome"; continue; fi
  c=$(echo "retrato de $nome em $CARIMBO" | git commit-tree "$arvore" -p "$pai")
  git update-ref "refs/salvas/$nome/$CARIMBO" "$c" && echo "SALVA: $nome -> $c"
done
