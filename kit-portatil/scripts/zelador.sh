#!/usr/bin/env bash
# ZELADOR: libera espaco sem nunca tocar no que esta em uso.
# Generalizado do `zelador.sh` do PhxSql (o original tem 594 linhas e sabe do
# cargo, dos bundles e das copias do provador; este e o nucleo).
#
# A REGRA que decide se ele ajuda ou destroi: **nada e apagado sem antes se
# provar que nenhum processo vivo esta usando aquilo** -- pelo `cwd`, pelos
# descritores abertos e pelos mapas de memoria de cada processo, nunca por
# data, nome ou palpite. E ele NAO mata processo nenhum: o processo pode ser
# de outro agente, e matar o servidor de um vizinho ja derrubou a propria
# sessao.
#
# USO
#   zelador.sh --ver                   so mostra o que faria (rode isto primeiro)
#   zelador.sh                         apaga
#   zelador.sh --alvos ARQ [--ver]     lista de alvos de outro arquivo
#
# OS ALVOS saem de `zelador.alvos` (no diretorio atual, ou --alvos), uma linha
# por alvo, `tipo | caminho-ou-glob | parametro`:
#
#   dir   | target/debug              |            apaga se ninguem usa
#   trava | target/debug/incremental  | target/debug/.cargo-lock
#                                       apaga SEGURANDO a trava da ferramenta
#                                       dona (flock -n); trava ausente = fica
#   tmp   | /tmp/meuprojeto-*         | 30         diretorio de teste solto:
#                                       fica se um numero no nome e PID vivo,
#                                       ou se mexido ha menos de N minutos
#
# TRES LICOES DO ORIGINAL, todas pagas:
#   1. O observador nao conta: o proprio zelador (e quem o chamou) tem o `cwd`
#      aqui. Sem excluir a LINHAGEM (ancestrais pelo ppid), ele respondia
#      «alguem trabalha aqui» sempre, com a maquina parada.
#   2. `cwd` diz quem PODE usar; a trava diz quem ESTA usando. Cache de
#      compilador se apaga com a trava do compilador na mao.
#   3. A primeira corrida achou 80.088 diretorios de teste soltos, 6,4 GB,
#      num disco a 560 MB livres. O liberado se MEDE no disco (df antes e
#      depois), nao se soma de estimativa.
#
# PORTAO DE QUANDO (opcional): se KIT_ESTA_MEDINDO apontar para um comando que
# sai 0 quando ha medicao em curso, o zelador RECUSA (saida 3) -- varrer
# gigabytes durante uma bancada reprova a bancada. `--mesmo-assim` passa por
# cima, para o disco acabando de verdade.
set -uo pipefail

VER=""; MESMO_ASSIM=""; ALVOS="./zelador.alvos"
while [ $# -gt 0 ]; do
  case "$1" in
    --ver) VER=1; shift ;;
    --mesmo-assim) MESMO_ASSIM=1; shift ;;
    --alvos) ALVOS="$2"; shift 2 ;;
    -h|--help) sed -n '2,45p' "$0"; exit 0 ;;
    *) echo "argumento desconhecido: $1" >&2; exit 2 ;;
  esac
done
[ -f "$ALVOS" ] || { echo "sem lista de alvos: $ALVOS (veja --help)" >&2; exit 2; }
BASE=$(cd "$(dirname "$ALVOS")" && pwd)
[ -n "$VER" ] && echo "== modo --ver: nada sera apagado =="

if [ -z "$VER" ] && [ -z "$MESMO_ASSIM" ] && [ -n "${KIT_ESTA_MEDINDO:-}" ] \
   && MEDINDO=$($KIT_ESTA_MEDINDO); then
  echo "== recusado: ha medicao em curso"
  printf '%s\n' "$MEDINDO" | head -3 | sed 's/^/   · /'
  exit 3
fi

# A linhagem do proprio zelador: ancestral nao conta, descendente conta.
PROPRIOS=" "
p=$$
while [ "${p:-0}" -gt 1 ]; do
  PROPRIOS="$PROPRIOS$p "
  [ -r "/proc/$p/stat" ] || break
  p=$(sed 's/.*) //' "/proc/$p/stat" 2>/dev/null | cut -d' ' -f2)
done

QUEM_SEGURA=""
# Em uso = algum processo vivo (fora da linhagem) com cwd, descritor ou mapa
# dentro do caminho. Guarda em QUEM_SEGURA o PID e o comando: «em uso» sem
# dizer por quem manda o leitor procurar sem saber o que.
em_uso() {
  local dir=$1 p cw
  QUEM_SEGURA=""
  for p in /proc/[0-9]*; do
    case "$PROPRIOS" in *" ${p##*/} "*) continue ;; esac
    cw=$(readlink "$p/cwd" 2>/dev/null) || continue
    case "$cw" in
      "$dir"|"$dir"/*) QUEM_SEGURA="PID ${p##*/} ($(tr -d '\0' < "$p/comm" 2>/dev/null)) cwd"; return 0 ;;
    esac
    if ls -l "$p/fd" 2>/dev/null | grep -qF -- "$dir/" \
       || grep -qF -- "$dir/" "$p/maps" 2>/dev/null; then
      QUEM_SEGURA="PID ${p##*/} ($(tr -d '\0' < "$p/comm" 2>/dev/null)) descritor/mapa"
      return 0
    fi
  done
  return 1
}

kb() { du -sk "$1" 2>/dev/null | cut -f1; }
mostra() { printf "  %-52s %6s MiB  %s\n" "$1" "$(( ${2:-0} / 1024 ))" "$3"; }

LIVRE_ANTES=$(df -k "$BASE" | awk 'NR==2{print $4}')
echo "== antes: $(df -h "$BASE" | awk 'NR==2{print $4}') livres"

while IFS= read -r linha || [ -n "$linha" ]; do
  case "$linha" in ''|'#'*) continue ;; esac
  tipo=$(printf '%s' "$linha" | cut -d'|' -f1 | xargs)
  alvo=$(printf '%s' "$linha" | cut -d'|' -f2 | xargs)
  param=$(printf '%s' "$linha" | cut -d'|' -f3- | xargs)
  case "$alvo" in /*) ;; *) alvo="$BASE/$alvo" ;; esac
  for a in $alvo; do
    [ -e "$a" ] || continue
    a=$(cd "$a" 2>/dev/null && pwd || echo "$a")
    t=$(kb "$a")
    case "$tipo" in
      dir)
        if em_uso "$a"; then mostra "$a" "$t" "EM USO por $QUEM_SEGURA, nao toco"; continue; fi
        mostra "$a" "$t" "ninguem usa"
        [ -n "$VER" ] || rm -rf "$a" ;;
      trava)
        trava="$param"; case "$trava" in /*) ;; *) trava="$BASE/$trava" ;; esac
        if [ ! -e "$trava" ]; then mostra "$a" "$t" "sem a trava $trava para conferir, fica"; continue; fi
        if ! flock -n "$trava" true 2>/dev/null; then mostra "$a" "$t" "trava tomada: usando AGORA, nao toco"; continue; fi
        mostra "$a" "$t" "trava livre"
        [ -n "$VER" ] || flock -n "$trava" rm -rf "$a" || echo "    trava tomada no meio, nao apagado" ;;
      tmp)
        min=${param:-30}
        vivo=""
        for parte in $(basename "$a" | tr -c '0-9\n' ' '); do
          [ -d "/proc/$parte" ] && vivo="$parte"
        done
        if [ -n "$vivo" ]; then mostra "$a" "$t" "PID $vivo vivo no nome, fica"; continue; fi
        if [ -n "$(find "$a" -maxdepth 0 -mmin -"$min" 2>/dev/null)" ]; then mostra "$a" "$t" "mexido ha menos de $min min, fica"; continue; fi
        if em_uso "$a"; then mostra "$a" "$t" "EM USO por $QUEM_SEGURA, nao toco"; continue; fi
        mostra "$a" "$t" "temporario solto"
        [ -n "$VER" ] || rm -rf "$a" ;;
      *) echo "  tipo desconhecido '$tipo' em: $linha" ;;
    esac
  done
done < "$ALVOS"

LIVRE_DEPOIS=$(df -k "$BASE" | awk 'NR==2{print $4}')
echo "== depois: $(df -h "$BASE" | awk 'NR==2{print $4}') livres"
if [ -n "$VER" ]; then
  echo "== --ver: nada apagado"
else
  echo "== liberou $(( (LIVRE_DEPOIS - LIVRE_ANTES) / 1024 )) MiB, medidos no disco"
fi
