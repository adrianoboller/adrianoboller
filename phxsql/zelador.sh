#!/usr/bin/env bash
# O zelador: libera espaco sem nunca tocar no que esta em uso.
#
# Existe porque o disco desta maquina ja chegou a 560 MB livres com cinco
# frentes compilando, e disco cheio nao da erro bonito -- da build morta pela
# metade e frente perdida.
#
# A REGRA que decide se ele ajuda ou destroi: **nada e apagado sem antes se
# provar que nenhum processo vivo esta usando aquilo**. Um zelador que apaga o
# `target` de quem esta compilando nao economiza espaco, perde uma rodada
# inteira de trabalho. Por isso cada worktree e conferida por processo com
# `cwd` dentro dela, e nao por data, nome ou palpite.
#
# E o que ele NUNCA faz, de proposito:
#   - nao mata processo nenhum (matar `phxsqld` de outro agente ja custou caro)
#   - nao apaga fonte, so artefato de compilacao e temporario conhecido
#   - nao apaga o pacote da versao corrente
#
#   ./zelador.sh            limpa
#   ./zelador.sh --ver      so mostra o que faria, sem apagar
set -uo pipefail
cd "$(dirname "$0")" || exit 1
RAIZ=$PWD
REPO=$(cd .. && pwd)
VER=${1:-}
[ "$VER" = "--ver" ] && echo "== modo --ver: nada sera apagado =="

kb() { du -sk "$1" 2>/dev/null | cut -f1; }
LIBEROU=0

apagar() {
  local alvo=$1 motivo=$2
  [ -e "$alvo" ] || return 0
  local t; t=$(kb "$alvo"); [ -z "$t" ] && return 0
  printf "  %-52s %6s MiB  %s\n" "${alvo#$REPO/}" "$((t/1024))" "$motivo"
  if [ "$VER" != "--ver" ]; then rm -rf "$alvo"; fi
  LIBEROU=$((LIBEROU+t))
}

# Um diretorio esta EM USO se algum processo vivo tem o `cwd` dentro dele. E a
# mesma conferencia que se faz antes de matar um servidor por PID -- por
# caminho real, nunca por nome de processo.
em_uso() {
  local dir=$1 p cw
  for p in /proc/[0-9]*; do
    cw=$(readlink "$p/cwd" 2>/dev/null) || continue
    case "$cw" in "$dir"|"$dir"/*) return 0 ;; esac
  done
  return 1
}

LIVRE_ANTES=$(df -k "$REPO" | awk 'NR==2{print $4}')
echo "== antes: $(df -h "$REPO" | awk 'NR==2{print $4}') livres"

echo "-- worktrees de agentes"
for w in "$REPO"/.claude/worktrees/*/; do
  [ -d "$w" ] || continue
  w=${w%/}
  if em_uso "$w"; then
    printf "  %-52s        EM USO, nao toco\n" "$(basename "$w")"
  else
    apagar "$w/phxsql/target" "agente terminou"
  fi
done

echo "-- arvore principal"
if em_uso "$RAIZ"; then
  echo "  compilando aqui agora, nao toco no target"
else
  apagar "$RAIZ/target/debug" "reconstroi em minutos"
  # Alvo cruzado ja virou pacote: o zip esta em pacotes/, o objeto nao serve
  # mais para nada ate a proxima geracao.
  for alvo in aarch64-unknown-linux-musl armv7-unknown-linux-musleabihf \
              x86_64-pc-windows-gnu x86_64-unknown-linux-gnu \
              x86_64-unknown-linux-musl aarch64-linux-android; do
    [ -d "$RAIZ/target/$alvo" ] && apagar "$RAIZ/target/$alvo" "ja empacotado"
  done
fi

echo "-- pacotes de versao antiga"
ATUAL=$(grep -m1 '^version' Cargo.toml | cut -d'"' -f2)
for z in pacotes/phxsql-*.zip; do
  [ -e "$z" ] || continue
  case "$z" in *"$ATUAL"*) ;; *) apagar "$z" "versao anterior a $ATUAL" ;; esac
done

echo "-- temporarios"
apagar "$RAIZ/target/tmp" "temporario de compilacao"

# A bateria deixa MILHARES de diretorios em /tmp -- 13.743 ocupando 4,7 GB na
# primeira vez que se olhou. Conferir um por um chamando `em_uso` varreria o
# /proc inteiro treze mil vezes; a lista de processos vivos se levanta UMA vez.
#
# Dois criterios guardam o que pode estar em uso, e os dois erram para o lado
# seguro: o nome carrega o PID de quem criou, entao PID vivo fica; e mexido nos
# ultimos 30 minutos fica, porque teste em curso ainda escreve.
python3 - "$VER" <<'FIM'
import os, sys, time, shutil, glob
so_ver = sys.argv[1] == "--ver"
vivos = set(os.listdir("/proc"))
corte = time.time() - 30 * 60
apagados = bytes_ = 0
guardados = 0
for d in glob.glob("/tmp/phxsql-*") + glob.glob("/tmp/phx-*"):
    partes = os.path.basename(d).split("-")
    pid_vivo = any(p.isdigit() and p in vivos for p in partes)
    try:
        novo_demais = os.path.getmtime(d) > corte
    except OSError:
        continue
    if pid_vivo or novo_demais:
        guardados += 1
        continue
    try:
        for raiz, _, arqs in os.walk(d):
            for a in arqs:
                try: bytes_ += os.path.getsize(os.path.join(raiz, a))
                except OSError: pass
        if not so_ver: shutil.rmtree(d, ignore_errors=True)
        apagados += 1
    except OSError:
        pass
print("  %d diretorios de teste soltos, %d MiB%s" %
      (apagados, bytes_ // (1024 * 1024), " (nao apagados: --ver)" if so_ver else ""))
if guardados:
    print("  %d guardados: PID vivo ou mexidos ha menos de 30 min" % guardados)
FIM

LIVRE_DEPOIS=$(df -k "$REPO" | awk 'NR==2{print $4}')
echo "== depois: $(df -h "$REPO" | awk 'NR==2{print $4}') livres"
# Medido no disco, e nao somado das partes: a soma a mao ja disse 362 MiB numa
# corrida que liberou quase 10 GB, porque nao enxergava o que o Python apagou.
echo "== liberou $(( (LIVRE_DEPOIS - LIVRE_ANTES) / 1024 )) MiB, medidos no disco"

# Processos do projeto que ficaram de pe: RELATA, nao mata. Matar o `phxsqld`
# de um agente vizinho ja aconteceu aqui e derrubou a propria sessao.
ORF=$(pgrep -x phxsqld 2>/dev/null | wc -l)
[ "$ORF" -gt 0 ] && echo "== aviso: $ORF phxsqld de pe (nao mato: podem ser de outro agente)"
exit 0
