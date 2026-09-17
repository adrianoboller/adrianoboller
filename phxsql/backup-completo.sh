#!/bin/sh
# Gera o PACOTE COMPLETO -- a lei, a historia e a arvore inteira -- e PROVA
# que ele restaura byte a byte.
#
#   ./backup-completo.sh [destino] [teto-de-arquivo]
#
# O `backup.sh` ao lado guarda a HISTORIA (o bundle da branch, provado
# restaurando). Ele nao guarda tres coisas, e este script existe por elas:
#
#   1. A LEI que mora fora do repositorio: `/root/.claude/CLAUDE.md` e o
#      unico arquivo desta casa que nao esta em lugar nenhum alem do disco
#      do conteiner -- e o conteiner e efemero.
#   2. Os arquivos, diretorios e subdiretorios que o git NAO rastreia:
#      resultados de bancada, pacotes, capturas, o que esta ignorado. O
#      bundle e a historia versionada; a arvore de trabalho e mais larga.
#   3. Diretorio VAZIO, que bundle nenhum carrega.
#
# O que fica de fora, e por que -- dito aqui e impresso a cada corrida, porque
# «gerador que faz menos do que o nome promete tem de dizer que fez menos»:
#
#   - `phxsql/target/`: compilado, 11 GB medidos em 17/09/2026; o `cargo`
#     refaz.
#   - `.git/`: a historia vai no bundle provado, que ENTRA no pacote.
#   - `__pycache__/` e `.claude/worktrees/`: derivados.
#   - arquivo maior que o TETO (64 MiB por padrao): e dado de bancada gerado
#     por script -- em 17/09 eram tres, `precos.{reg,ndx,log}`, 2,48 GB dos
#     2,9 GB da arvore. Cada um sai NOMEADO no manifesto, com o tamanho, e
#     `./backup-completo.sh . 0` desliga o teto e leva tudo.
#
# A prova e nos dois sentidos, como no `backup.sh`: nao basta o tar nao dar
# erro. O pacote e EXTRAIDO num diretorio de prova e cada arquivo e conferido
# por SHA-256 contra o disco -- os 3.089 de 17/09, um a um. Arquivo que faltar
# ou diferir reprova, nomeado.
set -eu

RAIZ=$(cd "$(dirname "$0")/.." && pwd)
DESTINO="${1:-$RAIZ}"
TETO="${2:-64M}"                       # 0 = sem teto
CARIMBO=$(date +%Y%m%d-%H%M)
PACOTE="$DESTINO/phxsql-completo-$CARIMBO.tar.gz"
LEI_GLOBAL="${HOME:-/root}/.claude/CLAUDE.md"

# O PID no nome e o que o zelador guarda: ele nunca apaga diretorio de processo
# vivo, e confere por caminho real, nunca por data ou nome.
PROVA="${TMPDIR:-/tmp}/phx-prova-do-completo-$$"
limpar() { rm -rf "$PROVA"; }
trap limpar EXIT INT TERM
mkdir -p "$PROVA/monta/lei" "$PROVA/extraido"

cd "$RAIZ"

echo "== 1/6 a historia: o bundle provado do backup.sh"
./phxsql/backup.sh "$(git rev-parse --abbrev-ref HEAD)" "$DESTINO" | tail -3
BUNDLE=$(ls -t "$DESTINO"/phxsql-*.bundle | head -1)

echo "== 2/6 a lei: os dois CLAUDE.md, com o SHA-256 de cada um"
[ -f "$LEI_GLOBAL" ] || { echo "REPROVOU: $LEI_GLOBAL nao existe" >&2; exit 1; }
cp "$LEI_GLOBAL" "$PROVA/monta/lei/CLAUDE-global.md"
cp "$RAIZ/CLAUDE.md" "$PROVA/monta/lei/CLAUDE-projeto.md"
( cd "$PROVA/monta/lei" && sha256sum CLAUDE-global.md CLAUDE-projeto.md > SHA256SUMS )

echo "== 3/6 a arvore: arquivos, diretorios e subdiretorios"
# Diretorios entram explicitamente (sem recursao) para que o VAZIO sobreviva.
# A poda e uma funcao, nao uma string: `eval` com parenteses quebra no dash.
varrer() {
    find . \( -path ./phxsql/target -o -path ./.git -o -name __pycache__ \
              -o -path ./.claude/worktrees \) -prune -o "$@"
}
varrer -type d -print > "$PROVA/dirs"
if [ "$TETO" = "0" ]; then
    varrer -type f -print > "$PROVA/arquivos"
    : > "$PROVA/grandes"
else
    varrer -type f ! -size +"$TETO" -print > "$PROVA/arquivos"
    varrer -type f -size +"$TETO" -printf '%s\t%p\n' > "$PROVA/grandes"
fi
# O pacote que acabou de nascer e o proprio destino deste tar ficam fora da
# lista: um tar que se inclui a si mesmo nunca termina igual.
grep -v -F -e "$(basename "$PACOTE")" "$PROVA/arquivos" > "$PROVA/arquivos.l" || true
mv "$PROVA/arquivos.l" "$PROVA/arquivos"
sort "$PROVA/dirs" "$PROVA/arquivos" > "$PROVA/lista"
N_DIRS=$(wc -l < "$PROVA/dirs"); N_ARQ=$(wc -l < "$PROVA/arquivos"); N_GRANDES=$(wc -l < "$PROVA/grandes")

echo "== 4/6 o manifesto"
{
    echo "PhxSql -- pacote completo, gerado por phxsql/backup-completo.sh"
    echo "quando:     $(date '+%Y-%m-%d %H:%M:%S %Z')"
    echo "branch:     $(git rev-parse --abbrev-ref HEAD)"
    echo "commit:     $(git rev-parse HEAD)"
    echo "historia:   $(basename "$BUNDLE") ($(du -h "$BUNDLE" | cut -f1), provado restaurando pelo backup.sh)"
    echo "lei:        lei/CLAUDE-global.md  <- $LEI_GLOBAL"
    echo "            lei/CLAUDE-projeto.md <- $RAIZ/CLAUDE.md"
    echo "arvore:     $N_ARQ arquivos, $N_DIRS diretorios (com os vazios)"
    echo
    echo "O QUE FICOU DE FORA -- isto nao e linha de exito:"
    echo "  phxsql/target/        $(du -sh phxsql/target 2>/dev/null | cut -f1)  compilado, o cargo refaz"
    echo "  .git/                 $(du -sh .git | cut -f1)  a historia esta no bundle acima"
    echo "  __pycache__/, .claude/worktrees/   derivados"
    if [ "$N_GRANDES" -gt 0 ]; then
        echo "  $N_GRANDES arquivo(s) acima do teto $TETO -- dado de bancada gerado por script:"
        awk -F'\t' '{printf "    %6.0f MiB  %s\n", $1/1048576, $2}' "$PROVA/grandes"
        echo "  (./backup-completo.sh <destino> 0 desliga o teto e leva tudo)"
    else
        echo "  nenhum arquivo acima do teto ($TETO)"
    fi
    echo
    echo "COMO RESTAURAR:"
    echo "  tar -xzf $(basename "$PACOTE")          # a arvore e a lei/"
    echo "  git clone --branch <branch> $(basename "$BUNDLE") phxsql-restaurado   # a historia"
    echo "  cp lei/CLAUDE-global.md ~/.claude/CLAUDE.md"
    echo "  sha256sum -c lei/SHA256SUMS"
} > "$PROVA/monta/MANIFESTO.txt"

echo "== 5/6 empacotando"
TAR="${PACOTE%.gz}"
tar -cf "$TAR" --no-recursion -T "$PROVA/lista"
tar -rf "$TAR" -C "$PROVA/monta" lei MANIFESTO.txt
gzip -f "$TAR"

echo "== 6/6 provando: extrair e conferir cada arquivo por SHA-256"
gzip -t "$PACOTE"
tar -xzf "$PACOTE" -C "$PROVA/extraido"
# Na arvore: a soma de cada arquivo da lista, conferida dentro do extraido.
xargs -d '\n' sha256sum < "$PROVA/arquivos" > "$PROVA/somas"
if ! ( cd "$PROVA/extraido" && sha256sum -c --quiet "$PROVA/somas" ); then
    echo "REPROVOU: arquivo faltando ou diferente no pacote extraido" >&2
    rm -f "$PACOTE"; exit 1
fi
# A lei: os dois CLAUDE.md batem com os originais, byte a byte.
( cd "$PROVA/extraido/lei" && sha256sum -c --quiet SHA256SUMS )
cmp -s "$LEI_GLOBAL" "$PROVA/extraido/lei/CLAUDE-global.md" || { echo "REPROVOU: CLAUDE-global.md difere" >&2; rm -f "$PACOTE"; exit 1; }
cmp -s "$RAIZ/CLAUDE.md" "$PROVA/extraido/lei/CLAUDE-projeto.md" || { echo "REPROVOU: CLAUDE-projeto.md difere" >&2; rm -f "$PACOTE"; exit 1; }
# Os diretorios, inclusive os vazios.
FALTA=0
while IFS= read -r d; do [ -d "$PROVA/extraido/$d" ] || { echo "  diretorio faltando: $d" >&2; FALTA=$((FALTA+1)); }; done < "$PROVA/dirs"
[ "$FALTA" -eq 0 ] || { echo "REPROVOU: $FALTA diretorio(s) faltando" >&2; rm -f "$PACOTE"; exit 1; }

echo
echo "PACOTE PROVADO: $PACOTE"
echo "  $(du -h "$PACOTE" | cut -f1), $N_ARQ arquivos e $N_DIRS diretorios conferidos por SHA-256, a lei byte a byte"
echo "  dentro: $(basename "$BUNDLE") (a historia), lei/ (os dois CLAUDE.md), MANIFESTO.txt"
if [ "$N_GRANDES" -gt 0 ]; then
    echo
    echo "FICARAM DE FORA $N_GRANDES arquivo(s) acima de $TETO -- nomeados no MANIFESTO.txt:"
    awk -F'\t' '{printf "  %6.0f MiB  %s\n", $1/1048576, $2}' "$PROVA/grandes"
fi
