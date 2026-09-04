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
#
# # O batimento de 15 min MORRE A CADA 30, e nao com a sessao
#
# Medido em 03/09/2026, duas vezes: o observador que roda este script a cada
# 900 s e criado com `persistent: true`, e a ferramenta responde
# `timeout 1800000ms` assim mesmo -- o teto de 30 minutos vale com persistente
# ou sem. Na pratica cada armacao rende DOIS avisos e para.
#
# Entao o gatilho de hora em hora nao e redundancia: ele e o unico motivo de o
# batimento fino voltar a existir, e quem o atender tem de REARMAR o observador,
# nao so rodar o script. Supor que `persistent` e persistente deixa o aviso
# calado parecendo que nao ha o que avisar -- que e o pior estado de um agente
# de comunicacao, e o mesmo defeito que este arquivo ja teve de outro jeito:
# silencio que passa por «tudo bem».

set -u
RAIZ="$(cd "$(dirname "$0")/.." && pwd)"
# Absoluto e tomado ANTES do `cd`, porque depois dele o `$0` relativo mente.
PORTAO="$(cd "$(dirname "$0")" && pwd)/bancada/esta-medindo.sh"
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

# A lista de cima NAO enxerga bancada, e nao deve: o crivo dela e o `comm`, e
# o `comm` de uma bancada e `python3` -- que nao distingue bancada de coisa
# nenhuma. Por o `python3` naquela lista faria o aviso apontar todo script de
# ninguem. Quem responde por essa metade e o portao, que cruza os dois crivos
# (nome do executavel onde ele diz algo, caminho do script onde nao diz) e
# exclui a si mesmo por LINHAGEM, nunca por texto.
#
# Sem ele o aviso dizia «nada compilando nem rodando agora» com uma bateria de
# concorrencia no meio da janela -- a pior mentira que este relatorio pode
# contar, porque e a que faz o proximo agente rodar por cima da medicao.
#
# E as duas metades se medem ANTES de qualquer uma imprimir. A primeira versao
# desta emenda imprimia o «nada rodando agora» e o «BANCADA MEDINDO» no mesmo
# relatorio, uma linha abaixo da outra: e o MESMO defeito que este arquivo ja
# pagou uma vez, quando o cabecalho dizia «3 processos» e a lista vinha vazia.
# Duas frases sobre o mesmo fato so nao se contradizem quando uma sabe da
# outra antes de falar.
MEDINDO=$("$PORTAO") || MEDINDO=''

if [ "${VIVOS:-0}" -gt 0 ]; then
  echo "⏳ em curso: $VIVOS processo(s) de compilacao, teste ou servidor"
  printf '%s\n' "$EM_CURSO" | head -4 \
    | awk '{t=$1; n=$2; $1="";$2=""; sub(/^  /,"");
            if (length($0)>66) $0=substr($0,1,66)"…"; print "   · "t"  "n"  "$0}'
elif [ -z "$MEDINDO" ]; then
  echo "· nada compilando nem rodando agora"
fi
if [ -n "$MEDINDO" ]; then
  echo "⏳ BANCADA MEDINDO — adie zelador, push e build ate ela acabar"
  printf '%s\n' "$MEDINDO" | head -3 \
    | awk -F'\t' '{if (length($3)>62) $3=substr($3,1,62)"…"; print "   · "$2"  "$3}'
fi
[ "$MEXIDOS" -gt 0 ] && echo "   · $MEXIDOS arquivo(s) mexidos e ainda nao commitados"
echo

# ------------------------------------------------------------- problemas
PROBLEMAS=0

# O push. Papel que nao esta cumprindo tem de aparecer como nao cumprindo --
# mas a MEDIDA e do estado, nunca do diagnostico guardado.
#
# Duas mentiras desta secao ja foram medidas e consertadas em 03/09/2026:
#
#   1. o contador usava `origin/HEAD`, que NUNCA foi definido neste clone --
#      entao o `||` caia para `git rev-list --count HEAD` e chamava os 392
#      commits do projeto inteiro de «sem enviar». Fallback que chuta e pior
#      que fallback que se cala: ele mente com numero grande e ar de medido.
#      Hoje a conta e contra o upstream de verdade, e sem upstream ele DIZ
#      que nao sabe;
#   2. a frase cravava «o push responde 403», que era verdade quando foi
#      escrita e deixou de ser. Limitacao registrada tambem envelhece, e a
#      prova ao lado e o que a conserva. Hoje, se ha commit por enviar, o
#      script PERGUNTA ao GitHub com o `--dry-run` -- que exerce o mesmo
#      `git-receive-pack` do push real sem mexer em nada -- e relata o que
#      voltou.
if git rev-parse --abbrev-ref '@{upstream}' >/dev/null 2>&1; then
  NAO_ENVIADOS=$(git rev-list --count '@{upstream}..HEAD' 2>/dev/null || echo '?')
else
  NAO_ENVIADOS='sem-upstream'
fi
if [ "$NAO_ENVIADOS" = "sem-upstream" ]; then
  echo "⚠️  branch sem upstream: nao da para dizer o que falta enviar"
  PROBLEMAS=$((PROBLEMAS+1))
elif [ "$NAO_ENVIADOS" != "0" ] && [ "$NAO_ENVIADOS" != "?" ]; then
  SECO=$(timeout 45 git push --dry-run 2>&1 | tail -3)
  case "$SECO" in
    *403*|*denied*|*rejected*|*"not authorized"*)
      echo "⚠️  $NAO_ENVIADOS commit(s) sem enviar — o push RECUSA:"
      echo "$SECO" | sed 's/^/   · /'
      ;;
    *)
      echo "⚠️  $NAO_ENVIADOS commit(s) sem enviar — o push NAO recusa; falta rodar"
      ;;
  esac
  PROBLEMAS=$((PROBLEMAS+1))
else
  echo "✅ nada por enviar: o origin esta na mesma ponta"
fi

# O backup sai do `if` do push, e isso e conserto de 03/09/2026: ele estava
# DENTRO do ramo «ha commit por enviar», entao no dia em que o push voltasse a
# funcionar o estado do pacote sumiria do aviso -- calado, e justamente quando
# o pacote vira segunda via em vez de unica saida.
ULTIMO=$(ls -t "$RAIZ"/*.bundle 2>/dev/null | head -1)
if [ -n "$ULTIMO" ]; then
  PONTA=$(git bundle list-heads "$ULTIMO" 2>/dev/null | head -1 | cut -c1-8)
  HEAD8=$(git rev-parse --short=8 HEAD)
  # QUANTO atrasado, e nao SE atrasado.
  #
  # A primeira versao acusava «ATRASADO» com UM commit de diferenca -- e como
  # todo commit deixa o pacote um atras, o alarme disparava o tempo todo. Um
  # alarme que se aprende a ignorar e pior que alarme nenhum, porque da a
  # sensacao de cobertura sem a cobertura.
  #
  # O criterio passa a ser material: mais de cinco commits, ou mais de duas
  # horas. Abaixo disso e informacao, e nao problema.
  ATRAS=$(git rev-list --count "$PONTA..HEAD" 2>/dev/null || echo 0)
  IDADE=$(( ( $(date +%s) - $(stat -c %Y "$ULTIMO") ) / 60 ))
  if [ "$ATRAS" = "0" ]; then
    echo "✅ backup em dia: $(basename "$ULTIMO")"
  elif [ "$ATRAS" -gt 5 ] || [ "$IDADE" -gt 120 ]; then
    echo "⚠️  backup ATRASADO em $ATRAS commit(s) e $IDADE min: $(basename "$ULTIMO")"
    echo "    refaca com ./backup.sh -- NUNCA com git bundle a mao, que grava no lugar errado"
    PROBLEMAS=$((PROBLEMAS+1))
  else
    echo "✅ backup a $ATRAS commit(s) e $IDADE min: $(basename "$ULTIMO")"
  fi
else
  echo "⚠️  nao ha pacote de backup nenhum"
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
