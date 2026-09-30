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
# Saida num arquivo, nao num pipe: `| tail -3` escondia o codigo de saida do backup.sh
# (o sh nao tem pipefail), e em 30/09 um bundle que NAO restaurava passou por aqui.
if ! ./phxsql/backup.sh "$(git rev-parse --abbrev-ref HEAD)" "$DESTINO" > "$PROVA/historia.log" 2>&1; then
    cat "$PROVA/historia.log" >&2
    echo "REPROVOU: o backup.sh falhou; nada foi empacotado" >&2
    exit 1
fi
tail -3 "$PROVA/historia.log"
BUNDLE=$(ls -t "$DESTINO"/phxsql-*.bundle | head -1)

echo "== 1b/6 a historia de TODAS as branches (o bundle acima leva so a atual)"
TODAS="$DESTINO/repositorio-todas-branches-$CARIMBO.bundle"
git bundle create "$TODAS" --all 2>/dev/null
git bundle verify "$TODAS" >/dev/null
git clone -q --mirror "$TODAS" "$PROVA/todas.git"
for b in $(git for-each-ref --format='%(refname:short)' refs/heads); do
    AQUI=$(git rev-parse "$b"); LA=$(git -C "$PROVA/todas.git" rev-parse "$b" 2>/dev/null || echo ausente)
    [ "$AQUI" = "$LA" ] || { echo "REPROVOU: branch $b difere no bundle ($AQUI x $LA)" >&2; rm -f "$TODAS"; exit 1; }
done
N_BRANCHES=$(git for-each-ref refs/heads | wc -l)
echo "  $N_BRANCHES branches conferidas pela ponta: $(basename "$TODAS") ($(du -h "$TODAS" | cut -f1))"

echo "== 2/6 a lei: os dois CLAUDE.md, com o SHA-256 de cada um"
# Sem a lei global o pacote AINDA sai (abortar tudo nao guardaria nada), mas a ausencia
# vai escrita no pacote, no manifesto e no fim da saida -- medido em 30/09: o conteiner
# foi recriado e o arquivo se perdeu, exatamente o risco que este passo existe para cobrir.
cp "$RAIZ/CLAUDE.md" "$PROVA/monta/lei/CLAUDE-projeto.md"
if [ -f "$LEI_GLOBAL" ]; then
    LEI_GLOBAL_OK=1
    cp "$LEI_GLOBAL" "$PROVA/monta/lei/CLAUDE-global.md"
    ( cd "$PROVA/monta/lei" && sha256sum CLAUDE-global.md CLAUDE-projeto.md > SHA256SUMS )
else
    LEI_GLOBAL_OK=0
    echo "  AUSENTE: $LEI_GLOBAL nao existe neste conteiner; o pacote leva so a lei do projeto" >&2
    echo "$LEI_GLOBAL nao existia quando este pacote foi gerado ($(date '+%Y-%m-%d %H:%M'))." \
        > "$PROVA/monta/lei/AUSENTE-CLAUDE-global.txt"
    ( cd "$PROVA/monta/lei" && sha256sum CLAUDE-projeto.md > SHA256SUMS )
fi

echo "== 3/6 a arvore: arquivos, diretorios e subdiretorios"
# Diretorios entram explicitamente (sem recursao) para que o VAZIO sobreviva.
# A poda e uma funcao, nao uma string: `eval` com parenteses quebra no dash.
varrer() {
    # todo target/ de cargo (o do phxclaw tinha 17 GB em 30/09 e a poda so conhecia o do
    # phxsql), e os derivados de npm e do Flutter
    find . \( -path ./.git -o -name target -o -name node_modules -o -name .dart_tool \
              -o -name __pycache__ -o -path ./.claude/worktrees \) -prune -o "$@"
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
    echo "adrianoboller (PhxSql + PhxClaw) -- pacote completo, gerado por phxsql/backup-completo.sh"
    echo "quando:     $(date '+%Y-%m-%d %H:%M:%S %Z')"
    echo "branch:     $(git rev-parse --abbrev-ref HEAD)"
    echo "commit:     $(git rev-parse HEAD)"
    echo "historia:   $(basename "$BUNDLE") ($(du -h "$BUNDLE" | cut -f1), provado restaurando pelo backup.sh)"
    echo "            $(basename "$TODAS") ($N_BRANCHES branches, cada ponta conferida)"
    if [ "$LEI_GLOBAL_OK" = 1 ]; then
    echo "lei:        lei/CLAUDE-global.md  <- $LEI_GLOBAL"
    else
    echo "lei:        lei/CLAUDE-global.md  AUSENTE -- $LEI_GLOBAL nao existia neste conteiner"
    fi
    echo "            lei/CLAUDE-projeto.md <- $RAIZ/CLAUDE.md"
    echo "arvore:     $N_ARQ arquivos, $N_DIRS diretorios (com os vazios)"
    echo
    echo "O QUE FICOU DE FORA -- isto nao e linha de exito:"
    echo "  */target/             $(du -shc phxsql/target phxclaw/target 2>/dev/null | tail -1 | cut -f1)  compilado, o cargo refaz"
    echo "  node_modules/, .dart_tool/   derivados do npm e do Flutter"
    echo "  fora do repositorio e NAO incluidos: /var/tmp/phxclaw-pg (PostgreSQL de teste),"
    echo "    /var/tmp/ollama-models, /opt/flutter, /opt/ollama -- ambiente, refeito por instalacao"
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
    echo "  git clone --mirror $(basename "$TODAS") repositorio.git               # todas as branches"
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
if [ "$LEI_GLOBAL_OK" = 1 ]; then
    cmp -s "$LEI_GLOBAL" "$PROVA/extraido/lei/CLAUDE-global.md" || { echo "REPROVOU: CLAUDE-global.md difere" >&2; rm -f "$PACOTE"; exit 1; }
fi
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
if [ "$LEI_GLOBAL_OK" = 0 ]; then
    echo
    echo "FALTOU A LEI GLOBAL: $LEI_GLOBAL nao existia; o pacote NAO a contem."
fi
