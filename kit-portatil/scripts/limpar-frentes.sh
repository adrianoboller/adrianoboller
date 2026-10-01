#!/bin/sh
# Apaga a copia de trabalho de frente INTEGRADA -- e so ela.
# Tres provas, todas obrigatorias (cognicao de 01/10/2026 05:00):
#   1. git status --porcelain VAZIO  (nada fora de commit: frente nao comita!)
#   2. a ponta do ramo alcancavel do HEAD
#   3. nenhum processo com cwd, descritor ou mapa la dentro
#   4. a copia NAO esta trancada (`git worktree lock`): a tranca e do agente
#      dono dela. Copia recem-criada e limpa e esta no HEAD -- passa nas
#      provas 1 e 2 -- e o agente pode ainda nao ter entrado nela (prova 3).
#      Medido em 01/10/2026: tres copias de agentes recem-lancados chegaram ao
#      `worktree remove`; so a tranca do git as segurou.
AQUI=$(cd "$(dirname "$0")" && pwd)
cd "$(git -C "$AQUI" rev-parse --show-toplevel)" || exit 1
# Antes de qualquer coisa, salva o trabalho de toda frente (ordem do dono, 01/10).
"$AQUI/salvar-frentes.sh" >/dev/null || { echo "salvar falhou: nada se apaga"; exit 1; }
usa(){ W=$1; for p in /proc/[0-9]*; do c=$(readlink $p/cwd 2>/dev/null); case "$c" in $W*) return 0;; esac; ls -l $p/fd 2>/dev/null | grep -q "$W" && return 0; grep -q "$W" $p/maps 2>/dev/null && return 0; done; return 1; }
for d in .claude/worktrees/agent-*; do
  [ -d "$d" ] || continue; w=${d##*agent-}; W=$PWD/$d
  if git worktree list --porcelain | grep -A4 "^worktree $W\$" | grep -q '^locked'; then echo "trancada (agente vivo), fica: $w"; continue; fi
  if [ -n "$(git -C "$W" status --porcelain 2>/dev/null)" ]; then echo "SUJA, fica: $w"; continue; fi
  git merge-base --is-ancestor "worktree-agent-$w" HEAD 2>/dev/null || { echo "nao integrada, fica: $w"; continue; }
  if usa "$W"; then echo "em uso, fica: $w"; continue; fi
  git worktree remove "$W" && git branch -D -q "worktree-agent-$w" && echo "removida: $w"
done
