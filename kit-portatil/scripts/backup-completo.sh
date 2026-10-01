#!/usr/bin/env bash
# BACKUP COMPLETO: historia + arvore inteira (com o nao versionado e os
# diretorios vazios) + arquivos que moram FORA do repositorio, e a PROVA de
# que tudo restaura byte a byte. Generalizado do `backup-completo.sh` do PhxSql.
#
# USO (de dentro do repositorio)
#   backup-completo.sh [DESTINO] [TETO]
#     DESTINO  onde gravar (padrao: a raiz do repositorio)
#     TETO     tamanho maximo de arquivo levado, no formato do `find -size`
#              (padrao 64M; 0 desliga o teto e leva tudo)
#
#   Variaveis:
#     KIT_EXCLUIR  caminhos relativos a raiz que ficam de fora, separados por
#                  espaco (padrao: ".git .claude/worktrees target node_modules")
#                  -- `__pycache__` fica sempre de fora
#     KIT_FORA     arquivos FORA do repositorio que entram no pacote em `fora/`
#                  (padrao: "$HOME/.claude/CLAUDE.md", a lei global)
#     KIT_PREFIXO  prefixo do nome (padrao: nome do repositorio)
#
# Precisa do `backup.sh` deste kit ao lado (a historia vem dele, ja provada).
#
# O QUE FICA DE FORA E DITO: cada exclusao, cada arquivo acima do teto (com o
# tamanho) e cada arquivo de KIT_FORA que nao existe saem NOMEADOS na saida e
# no MANIFESTO.txt, sob um cabecalho que nao e linha de exito. Foi assim que se
# descobriu, num conteiner recriado, que a lei global morava so no disco dele
# e nao tinha copia em lugar nenhum: o pacote sai assim mesmo, dizendo que
# ela FALTOU, em vez de reprovar o backup inteiro.
#
# A PROVA: o pacote e extraido num diretorio de prova e CADA arquivo e
# conferido por SHA-256 contra o disco; os de `fora/` por `cmp`; os
# diretorios (inclusive vazios) por existencia. Arquivo que faltar ou
# diferir reprova, nomeado, e o pacote e apagado.
set -eu

AQUI=$(cd "$(dirname "$0")" && pwd)
RAIZ=$(git rev-parse --show-toplevel)
DESTINO="${1:-$RAIZ}"
TETO="${2:-64M}"
EXCLUIR="${KIT_EXCLUIR:-.git .claude/worktrees target node_modules}"
FORA="${KIT_FORA:-${HOME:-/root}/.claude/CLAUDE.md}"
PREFIXO="${KIT_PREFIXO:-$(basename "$RAIZ")}"
CARIMBO=$(date +%Y%m%d-%H%M)
PACOTE="$DESTINO/$PREFIXO-completo-$CARIMBO.tar.gz"

PROVA="${TMPDIR:-/tmp}/kit-prova-do-completo-$$"
limpar() { rm -rf "$PROVA"; }
trap limpar EXIT INT TERM
mkdir -p "$PROVA/monta/fora" "$PROVA/extraido"
cd "$RAIZ"

echo "== 1/6 a historia: o bundle provado do backup.sh"
KIT_PREFIXO="$PREFIXO" "$AQUI/backup.sh" "$(git rev-parse --abbrev-ref HEAD)" "$DESTINO" | tail -3
BUNDLE=$(ls -t "$DESTINO/$PREFIXO"-*.bundle | head -1)

echo "== 2/6 os arquivos de fora do repositorio"
PRESENTES=""; AUSENTES=""
: > "$PROVA/monta/fora/ORIGEM"
for f in $FORA; do
    if [ -f "$f" ]; then
        nome=$(printf '%s' "$f" | sed 's#^/##; s#/#__#g')
        cp "$f" "$PROVA/monta/fora/$nome"
        printf '%s\t%s\n' "$nome" "$f" >> "$PROVA/monta/fora/ORIGEM"
        PRESENTES="$PRESENTES $nome"
    else
        echo "   AUSENTE: $f -- o pacote sai SEM ele"
        AUSENTES="$AUSENTES $f"
    fi
done
( cd "$PROVA/monta/fora" && { [ -z "$PRESENTES" ] && : > SHA256SUMS || sha256sum $PRESENTES > SHA256SUMS; } )

echo "== 3/6 a arvore: arquivos, diretorios e subdiretorios"
poda=( \( -name __pycache__ )
for e in $EXCLUIR; do poda+=( -o -path "./$e" ); done
poda+=( \) -prune -o )
find . "${poda[@]}" -type d -print > "$PROVA/dirs"
if [ "$TETO" = "0" ]; then
    find . "${poda[@]}" -type f -print > "$PROVA/arquivos"
    : > "$PROVA/grandes"
else
    find . "${poda[@]}" -type f ! -size +"$TETO" -print > "$PROVA/arquivos"
    find . "${poda[@]}" -type f -size +"$TETO" -printf '%s\t%p\n' > "$PROVA/grandes"
fi
# Um tar que se inclui a si mesmo nunca termina igual.
grep -v -F -e "$(basename "$PACOTE")" "$PROVA/arquivos" > "$PROVA/arquivos.l" || true
mv "$PROVA/arquivos.l" "$PROVA/arquivos"
sort "$PROVA/dirs" "$PROVA/arquivos" > "$PROVA/lista"
N_DIRS=$(wc -l < "$PROVA/dirs"); N_ARQ=$(wc -l < "$PROVA/arquivos"); N_GRANDES=$(wc -l < "$PROVA/grandes")

echo "== 4/6 o manifesto"
{
    echo "pacote completo, gerado por backup-completo.sh (kit portatil)"
    echo "quando:     $(date '+%Y-%m-%d %H:%M:%S %Z')"
    echo "branch:     $(git rev-parse --abbrev-ref HEAD)"
    echo "commit:     $(git rev-parse HEAD)"
    echo "historia:   $(basename "$BUNDLE") ($(du -h "$BUNDLE" | cut -f1), provado restaurando)"
    echo "arvore:     $N_ARQ arquivos, $N_DIRS diretorios (com os vazios)"
    echo "fora/:      $(cut -f2 "$PROVA/monta/fora/ORIGEM" | tr '\n' ' ')"
    echo
    echo "O QUE FICOU DE FORA -- isto nao e linha de exito:"
    for e in $EXCLUIR; do
        [ -e "$e" ] && echo "  $e  $(du -sh "$e" 2>/dev/null | cut -f1)"
    done
    echo "  __pycache__/"
    for a in $AUSENTES; do echo "  AUSENTE: $a (nao existia nesta maquina)"; done
    if [ "$N_GRANDES" -gt 0 ]; then
        echo "  $N_GRANDES arquivo(s) acima do teto $TETO:"
        awk -F'\t' '{printf "    %6.0f MiB  %s\n", $1/1048576, $2}' "$PROVA/grandes"
        echo "  (backup-completo.sh <destino> 0 desliga o teto)"
    else
        echo "  nenhum arquivo acima do teto ($TETO)"
    fi
    echo
    echo "COMO RESTAURAR:"
    echo "  tar -xzf $(basename "$PACOTE")"
    echo "  git clone --branch <branch> $(basename "$BUNDLE") restaurado"
    echo "  (cd fora && sha256sum -c SHA256SUMS); a origem de cada um esta em fora/ORIGEM"
} > "$PROVA/monta/MANIFESTO.txt"

echo "== 5/6 empacotando"
TAR="${PACOTE%.gz}"
tar -cf "$TAR" --no-recursion -T "$PROVA/lista"
tar -rf "$TAR" -C "$PROVA/monta" fora MANIFESTO.txt
gzip -f "$TAR"

echo "== 6/6 provando: extrair e conferir cada arquivo por SHA-256"
gzip -t "$PACOTE"
tar -xzf "$PACOTE" -C "$PROVA/extraido"
xargs -d '\n' sha256sum < "$PROVA/arquivos" > "$PROVA/somas"
if ! ( cd "$PROVA/extraido" && sha256sum -c --quiet "$PROVA/somas" ); then
    echo "REPROVOU: arquivo faltando ou diferente no pacote extraido" >&2
    rm -f "$PACOTE"; exit 1
fi
( cd "$PROVA/extraido/fora" && sha256sum -c --quiet SHA256SUMS )
while IFS="$(printf '\t')" read -r nome origem; do
    [ -n "$nome" ] || continue
    cmp -s "$origem" "$PROVA/extraido/fora/$nome" || { echo "REPROVOU: $origem difere" >&2; rm -f "$PACOTE"; exit 1; }
done < "$PROVA/monta/fora/ORIGEM"
FALTA=0
while IFS= read -r d; do [ -d "$PROVA/extraido/$d" ] || { echo "  diretorio faltando: $d" >&2; FALTA=$((FALTA+1)); }; done < "$PROVA/dirs"
[ "$FALTA" -eq 0 ] || { echo "REPROVOU: $FALTA diretorio(s) faltando" >&2; rm -f "$PACOTE"; exit 1; }

echo
echo "PACOTE PROVADO: $PACOTE"
echo "  $(du -h "$PACOTE" | cut -f1), $N_ARQ arquivos e $N_DIRS diretorios conferidos por SHA-256"
for a in $AUSENTES; do echo "  FALTOU: $a -- nao existia nesta maquina"; done
if [ "$N_GRANDES" -gt 0 ]; then
    echo
    echo "FICARAM DE FORA $N_GRANDES arquivo(s) acima de $TETO -- nomeados no MANIFESTO.txt"
fi
