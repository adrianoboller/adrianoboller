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

# Os bundles do backup: 28 arquivos de ~19 MiB cresciam sem ninguem olhar, e o
# zelador nao os enxergava -- a guarda existia, mas o alcance dela parava no
# `target` e nos worktrees. Backup, porem, e a ultima coisa que um zelador pode
# apagar por palpite, entao aqui a prova e por OBJETO, nunca por data:
#
#   1. nenhum processo vivo tem o arquivo aberto (por caminho real, em /proc)
#   2. TODA cabeca do bundle antigo e alcancavel a partir das cabecas do bundle
#      mais novo -- ou seja, ele nao carrega um commit que o novo perdeu
#
# O criterio 2 e o que importa: bundle de uma frente descartada guarda commit
# que nao esta mais em ref nenhuma, e esse FICA. Foi para isso que ele existe.
#
# Duas geracoes sobrevivem, e nao uma: bundle corrompido nao avisa antes da
# hora em que se precisa dele.
echo "-- bundles do backup"
python3 - "$VER" "$REPO" <<'FIM'
import os, re, subprocess, sys, glob

so_ver = sys.argv[1] == "--ver"
repo = sys.argv[2]

def git(*args, cwd=repo):
    return subprocess.run(("git",) + args, cwd=cwd, capture_output=True, text=True)

# So os que o proprio script de backup gera entram na rotacao. Bundle com nome
# escolhido a mao -- `guardado.bundle` -- e decisao de alguem, e decisao nao se
# desfaz por rotina, mesmo com a prova de que o conteudo esta contido: o que
# esta guardado ali e o NOME, nao o objeto.
gerado = re.compile(r"^phxsql-\d{8}-\d{4}\.bundle$")
bundles = sorted(
    (b for b in glob.glob(os.path.join(repo, "*.bundle"))
     if gerado.match(os.path.basename(b))),
    key=os.path.getmtime,
)
if len(bundles) < 3:
    print("  %d bundles, nada a fazer (duas geracoes ficam sempre)" % len(bundles))
    raise SystemExit

novo = bundles[-1]
if git("bundle", "verify", novo).returncode != 0:
    print("  o bundle mais novo NAO passa no verify -- nao apago nada")
    raise SystemExit

def cabecas(b):
    r = git("bundle", "list-heads", b)
    return [l.split()[0] for l in r.stdout.splitlines() if l.strip()]

# O conjunto de tudo o que o bundle novo alcanca. Uma varredura so, e nao uma
# por bundle: `rev-list` com 30 cabecas custa o mesmo que com uma.
alcanca = set(git("rev-list", *cabecas(novo)).stdout.split())

# Arquivos abertos por processo vivo, por caminho real -- a mesma conferencia
# que `em_uso` faz para diretorio, so que por descritor em vez de cwd.
abertos = set()
for p in os.listdir("/proc"):
    if not p.isdigit():
        continue
    d = "/proc/%s/fd" % p
    try:
        for fd in os.listdir(d):
            try:
                abertos.add(os.readlink(os.path.join(d, fd)))
            except OSError:
                pass
    except OSError:
        pass

apagados = bytes_ = 0
for b in bundles[:-2]:          # as duas geracoes mais novas nunca entram
    nome = os.path.basename(b)
    if b in abertos:
        print("  %-34s ABERTO por processo vivo, nao toco" % nome)
        continue
    sobra = [c for c in cabecas(b) if c not in alcanca]
    if sobra:
        print("  %-34s GUARDO: %d commit(s) que o novo nao tem" % (nome, len(sobra)))
        continue
    t = os.path.getsize(b)
    print("  %-34s %4d MiB  contido no mais novo" % (nome, t // (1024 * 1024)))
    if not so_ver:
        os.remove(b)
    apagados += 1
    bytes_ += t
print("  %d bundles, %d MiB%s" %
      (apagados, bytes_ // (1024 * 1024), " (nao apagados: --ver)" if so_ver else ""))
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
