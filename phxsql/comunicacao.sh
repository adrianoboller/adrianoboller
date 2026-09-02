#!/bin/bash
# O agente de comunicacao: diz o que esta sendo feito, e o que quebrou.
#
# Ele MEDE, nao narra. A diferenca importa: um aviso escrito de memoria conta o
# que eu lembro de ter feito, e o que eu lembro e o que eu acho que fiz. Este
# aqui le o git, os processos vivos, o disco e a ultima corrida de portao -- e
# quando nao consegue medir alguma coisa, ele DIZ que nao conseguiu, em vez de
# calar e parecer que esta tudo bem.
#
#   ./comunicacao.sh          o aviso desde o ultimo aviso
#   ./comunicacao.sh --zerar  esquece a marca e conta o dia inteiro
#
# A marca do ultimo aviso mora fora do repositorio, para nao sujar o diff nem
# virar conflito entre frentes.

set -u
RAIZ="$(cd "$(dirname "$0")/.." && pwd)"
MARCA="${TMPDIR:-/tmp}/phx-comunicacao-marca"
AGORA="$(date '+%d/%m/%Y %H:%M')"

[ "${1:-}" = "--zerar" ] && rm -f "$MARCA"

cd "$RAIZ" || { echo "nao achei $RAIZ"; exit 2; }

DESDE=""
[ -f "$MARCA" ] && DESDE="$(cat "$MARCA")"
# Commit que ainda existe? Marca de sessao anterior aponta para nada.
if [ -n "$DESDE" ] && ! git cat-file -e "$DESDE^{commit}" 2>/dev/null; then
  DESDE=""
fi

echo "PhxSql — $AGORA"
echo

# ----------------------------------------------------------- o que andou
if [ -n "$DESDE" ]; then
  N=$(git rev-list --count "$DESDE"..HEAD 2>/dev/null || echo 0)
  FAIXA="$DESDE..HEAD"
  ROTULO="desde o ultimo aviso"
else
  N=$(git log --since='today 00:00' --oneline | wc -l)
  FAIXA=""
  ROTULO="hoje"
fi

if [ "$N" -gt 0 ]; then
  echo "✅ $N commit(s) $ROTULO:"
  # No maximo oito: um aviso de quinze em quinze minutos que despeja o dia
  # inteiro nao e aviso, e parede -- e parede ninguem le.
  if [ -n "$FAIXA" ]; then
    git log --format='   · %h %s (%ad)' --date=format:'%H:%M' "$FAIXA" | head -8
  else
    git log --since='today 00:00' --format='   · %h %s (%ad)' --date=format:'%H:%M' | head -8
  fi
  [ "$N" -gt 8 ] && echo "   · … e mais $((N-8)), veja com: git log --oneline"
  true
else
  echo "· nenhum commit $ROTULO"
fi
echo

# --------------------------------------------------- o que esta em curso
# UM CRIVO SO, E UMA FONTE SO.
#
# A primeira versao contava com um `grep` e listava com outro, e os dois
# discordavam: o cabecalho dizia «3 processos» e a lista vinha VAZIA. O
# mentiroso era a contagem -- o padrao contem a palavra `phxsqld`, e o proprio
# `grep` aparece no `ps` carregando o padrao na linha de comando, entao ele
# casava a si mesmo.
#
# E a TERCEIRA vez que esta armadilha aparece nesta base: ja pegou um
# `pgrep -f cacar2` e um `pgrep -f video-demonstracao`, e nas duas vezes o
# relato disse «esta rodando» enquanto nada rodava. Aqui ela reapareceu no
# proprio agente que existe para relatar -- que e o pior lugar possivel.
#
# O conserto e estrutural, e nao um `grep -v grep` a mais: a lista se mede UMA
# vez, e a contagem sai dela. Duas medicoes da mesma coisa e uma a mais do que
# se pode manter em acordo.
# E o crivo e o NOME DO EXECUTAVEL, nunca a linha de comando.
#
# A segunda versao ainda errava, e errava mais fundo: filtrando pela linha de
# comando, ela casava o proprio shell que a invocou -- porque essa linha
# carrega o comando inteiro que eu tinha acabado de digitar, `cargo build`
# incluido. Quem pergunta «quem esta compilando?» olhando texto acaba se
# achando.
#
# `comm` e o nome do binario: o shell chama-se `bash` e nunca `cargo`, entao
# ele sai por construcao, e nao por uma excecao que alguem lembrou de escrever.
EM_CURSO=$(ps -eo etime=,comm=,args= --sort=-etime \
  | awk '$2=="cargo"||$2=="rustc"||$2=="node"||$2=="phxsqld"' || true)
VIVOS=$(printf '%s' "$EM_CURSO" | grep -c . || true)
MEXIDOS=$(git status --porcelain | wc -l)
if [ "${VIVOS:-0}" -gt 0 ]; then
  echo "⏳ em curso: $VIVOS processo(s) de compilacao, teste ou servidor"
  printf '%s\n' "$EM_CURSO" | head -4 \
    | awk '{t=$1; n=$2; $1="";$2=""; sub(/^  /,"");
            if (length($0)>66) $0=substr($0,1,66)"…"; print "   · "t"  "n"  "$0}'
else
  echo "· nada compilando nem rodando agora"
fi
[ "$MEXIDOS" -gt 0 ] && echo "   · $MEXIDOS arquivo(s) mexidos e ainda nao commitados"
echo

# ------------------------------------------------------------- problemas
PROBLEMAS=0

# O push. E o problema conhecido desta sessao, e ele SEMPRE aparece enquanto
# durar -- papel que nao esta cumprindo tem de aparecer como nao cumprindo.
NAO_ENVIADOS=$(git rev-list --count origin/HEAD..HEAD 2>/dev/null \
             || git rev-list --count HEAD 2>/dev/null || echo '?')
if [ "$NAO_ENVIADOS" != "0" ] && [ "$NAO_ENVIADOS" != "?" ]; then
  echo "⚠️  $NAO_ENVIADOS commit(s) sem enviar — o push responde 403 por identidade da sessao"
  ULTIMO=$(ls -t "$RAIZ"/*.bundle 2>/dev/null | head -1)
  if [ -n "$ULTIMO" ]; then
    PONTA=$(git bundle list-heads "$ULTIMO" 2>/dev/null | head -1 | cut -c1-8)
    HEAD8=$(git rev-parse --short=8 HEAD)
    if [ "$PONTA" = "$HEAD8" ]; then
      echo "   · backup em dia: $(basename "$ULTIMO")"
    else
      echo "   ⚠️ backup ATRASADO: $(basename "$ULTIMO") para na $PONTA, o topo e $HEAD8"
      PROBLEMAS=$((PROBLEMAS+1))
    fi
  else
    echo "   ⚠️ nao ha pacote de backup nenhum"
    PROBLEMAS=$((PROBLEMAS+1))
  fi
  PROBLEMAS=$((PROBLEMAS+1))
fi

# Disco. O limite de 2 GB e o mesmo que o zelador usa para avisar.
LIVRE_MB=$(df -Pm /home/user | awk 'NR==2{print $4}')
if [ "$LIVRE_MB" -lt 2048 ]; then
  echo "⚠️  disco: ${LIVRE_MB} MiB livres — abaixo do limite de 2 GiB"
  PROBLEMAS=$((PROBLEMAS+1))
else
  echo "✅ disco: $((LIVRE_MB/1024)) GiB livres"
fi

# Binario velho contra a interface: a armadilha que ja custou uma rodada.
BIN="$RAIZ/phxsql/target/release/phxsqld"
UI="$RAIZ/phxsql/crates/phxsql-server/ui"
if [ -x "$BIN" ] && [ -d "$UI" ]; then
  NOVO=$(find "$UI" -newer "$BIN" -type f | head -1)
  if [ -n "$NOVO" ]; then
    echo "⚠️  binario mais VELHO que $(basename "$NOVO") — medir agora mediria o passado"
    PROBLEMAS=$((PROBLEMAS+1))
  fi
fi

# Sintaxe da interface: 200 ms, e pega o defeito que ja derrubou 31 casos.
if command -v node >/dev/null 2>&1 && [ -d "$UI" ]; then
  QUEBRADOS=0
  for f in "$UI"/*.js; do
    [ -e "$f" ] || continue
    node --check "$f" >/dev/null 2>&1 || { echo "⚠️  $(basename "$f") NAO compila"; QUEBRADOS=1; }
  done
  [ "$QUEBRADOS" = 1 ] && PROBLEMAS=$((PROBLEMAS+1))
fi

[ "$PROBLEMAS" = 0 ] && echo "✅ nenhum problema novo medido"

echo
echo "— medido em $AGORA. O que este aviso NAO mede: se a suite de testes"
echo "  esta verde (roda-la a cada 15 min competiria com o trabalho)."

git rev-parse HEAD > "$MARCA"
