#!/usr/bin/env bash
# O disco que RECUSA, contra o sistema operacional -- pedidos 509, 512, 522 e
# 533.
#
#   sudo bancada/catastrofes/prova.sh            constroi o executor e roda
#   P=/caminho/do/binario bancada/catastrofes/prova.sh   usa um binario pronto
#   RODADAS=3 SAIDA=arquivo.json ...              quantas vezes, e onde gravar
#
# O que depende do sistema operacional se prova contra o sistema operacional:
# os testes de `crates/phxsql-store/tests/disco-que-recusa.rs` FORJAM a recusa
# (EIO no fsync, ENOSPC na pagina); este roteiro faz o disco recusar de
# verdade, com as receitas do Apendice A do parecer do papel C
# (`docs/propostas/parecer-dba-496-catastrofes-2026-09-24.md`):
#
#   512 (C2b)  tmpfs de 512 KiB: o disco enche no meio da carga, e o `.ndx`
#              fecha no mesmo punho (o caminho do `gravar_de_verdade`) ou so
#              pelo `Drop` (SEM_SYNC). Depois o tmpfs cresce e se confere.
#   509 (C1)   ext4 sobre loop com PROVISIONAMENTO FINO (todo bloco existe, os
#              livres do ext4 viram buraco num tmpfs cheio): o `write` passa,
#              a escrita de fundo falha, o 1o fsync recusa. Os dois fechos no
#              MESMO processo, como o servidor faz; e com ABORTA=1, o gancho do
#              servidor (o abort na recusa). Depois remonta e confere.
#   522 (P4)   o mesmo provisionamento fino, e NENHUM fsync do motor: insere,
#              fecha pelo `Drop` (o `fechar`) ou cai sem ele (o controle),
#              `syncfs` com o disco cheio, libera, remonta e confere. O que se
#              mede e o byte 52 que o disco guardou -- o `fechar` baixava o
#              byte sem `fsync`, e o nucleo guardava o cabecalho limpo sobre
#              paginas perdidas.
#   533 (C4)   a QUEDA DE ENERGIA com a ordem escolhida (`queda.py`): o `.reg`
#              chega; do `.ndx` nada, ou so as paginas a partir da 1; e o
#              ext4 cai sem descarregar o resto (`FS_IOC_SHUTDOWN`). O que se
#              mede e o byte 52 que o disco guardou e o `excluir` do pai com
#              filhas -- sem o `fdatasync` da subida, o disco guardava o 0 do
#              ultimo fecho e o pai com filhas se apagava calado.
#   CENARIOS="522" roda so o 522: e o que compara o binario de antes com o de
#              depois, porque os modos do 509 exigem o gancho que so existe
#              desde o 509. O mesmo vale para CENARIOS="533".
#
# Precisa de root e `unshare -m`: toda montagem nasce e morre num espaco de
# montagem privado, e nada toca o disco da maquina. Sem privilegio o roteiro
# diz NAO PROVADO e sai com 2 -- nunca verde por omissao.
set -u
AQUI="$(cd "$(dirname "$0")" && pwd)"
RAIZ="$(cd "$AQUI/../.." && pwd)"
RODADAS="${RODADAS:-3}"
SAIDA="${SAIDA:-$AQUI/resultados.json}"

if [ -z "${DENTRO_DO_UNSHARE:-}" ]; then
  if [ "$(id -u)" != 0 ] || ! unshare -m --propagation private true 2>/dev/null; then
    echo "NAO PROVADO: precisa de root e de unshare -m (CAP_SYS_ADMIN)."
    exit 2
  fi
  if [ -z "${P:-}" ]; then
    (cd "$RAIZ" && CARGO_PROFILE_DEV_DEBUG=0 cargo build --offline -q \
      -p phxsql-store --example disco-que-recusa) || exit 1
    P="$RAIZ/target/debug/examples/disco-que-recusa"
  fi
  export P RODADAS SAIDA DENTRO_DO_UNSHARE=1
  exec unshare -m --propagation private bash "$0" "$@"
fi

S="$(mktemp -d "${TMPDIR:-/tmp}/phx-catastrofes-XXXXXX")"
mkdir -p "$S/p1" "$S/back" "$S/ext"
LOOP=""
limpar() {
  umount "$S/ext" 2>/dev/null
  [ -n "$LOOP" ] && losetup -d "$LOOP" 2>/dev/null
  umount "$S/back" "$S/p1" 2>/dev/null
  rm -rf "$S"
}
trap limpar EXIT

# Uma linha `chave=valor ...` por passo; o JSON sai delas, sem numero digitado.
REG="$S/registro.txt"
: > "$REG"
# O diretorio temporario sai do texto: ele morre com a corrida, e o caminho
# so faria o resultado mudar de uma corrida para a outra sem nada ter mudado.
anotar() { local t="${2//$S\//}"; echo "$1|$t" >> "$REG"; echo "  [$1] $t"; }

# ---------------------------------------------------------------- 512 (C2b)
c2b() {
  local rotulo="$1" sem_sync="$2" r
  for r in $(seq 1 "$RODADAS"); do
    mount -t tmpfs -o size=512k tmpfs "$S/p1"
    while IFS= read -r l; do anotar "$rotulo#$r" "$l"; done < <(SEM_SYNC="$sem_sync" "$P" enospc "$S/p1/db" 20000 2>&1)
    mount -o remount,size=64m "$S/p1"
    while IFS= read -r l; do anotar "$rotulo#$r" "$l"; done < <("$P" conferir "$S/p1/db" 2>&1)
    umount "$S/p1"
  done
}

# ------------------------------------------------------------------ 509 (C1)
# O provisionamento fino fiel do `p3b.sh`: a imagem inteira alocada, e so os
# blocos LIVRES do ext4 viram buraco num tmpfs que em seguida enche.
c1() {
  local rotulo="$1" aborta="$2" r img="$S/back/disco.img" livre saida
  for r in $(seq 1 "$RODADAS"); do
    mount -t tmpfs -o size=80m tmpfs "$S/back"
    truncate -s 64M "$img"
    mkfs.ext4 -q -F -N 64 -J size=4 "$img"
    fallocate -l 64M "$img"
    LOOP="$(losetup -f --show "$img")"
    mount -t ext4 "$LOOP" "$S/ext"
    "$P" criar "$S/ext/db"
    sync
    umount "$S/ext"
    for faixa in $(dumpe2fs "$img" 2>/dev/null | sed -n 's/^ *Free blocks: //p' | tr ',' '\n' | tr -d ' '); do
      [ -z "$faixa" ] && continue
      local a="${faixa%-*}" b="${faixa#*-}"
      fallocate -p -o $((a * 4096)) -l $(((b - a + 1) * 4096)) "$img"
    done
    livre="$(df -k --output=avail "$S/back" | tail -1)"
    dd if=/dev/zero of="$S/back/enchimento" bs=1k count=$((livre - 64)) 2>/dev/null
    mount -t ext4 "$LOOP" "$S/ext"
    while IFS= read -r l; do anotar "$rotulo#$r" "$l"; done < <("$P" inserir "$S/ext/db" 5000 2>&1)
    # O byte 52 ANTES de qualquer fecho: o processo que inseriu ja saiu, e o
    # `fechar` dele (sem fsync, por desenho) ja o gravou no cache do nucleo.
    while IFS= read -r l; do anotar "$rotulo#$r" "antes $l"; done < <(SO_LER=1 "$P" conferir "$S/ext/db" 2>&1 | grep '^byte52=')
    saida="$S/fecho.txt"
    rm -f "$S/sinal"
    ABORTA="$aborta" "$P" fecho2x "$S/ext/db" "$S/sinal" > "$saida" 2>&1 &
    local pid=$!
    # O fecho 1 acontece com o disco cheio; so depois o «operador» libera
    # espaco e manda o mesmo processo fazer o fecho 2.
    for _ in $(seq 1 500); do
      grep -q '^fecho1=' "$saida" && break
      kill -0 "$pid" 2>/dev/null || break
      sleep 0.02
    done
    rm -f "$S/back/enchimento"
    touch "$S/sinal"
    wait "$pid"
    local rc=$?
    while IFS= read -r l; do anotar "$rotulo#$r" "$l"; done < "$saida"
    anotar "$rotulo#$r" "saida=$rc"
    while IFS= read -r l; do anotar "$rotulo#$r" "cache $l"; done < <(SO_LER=1 "$P" conferir "$S/ext/db" 2>&1)
    umount "$S/ext"
    mount -t ext4 "$LOOP" "$S/ext"
    while IFS= read -r l; do anotar "$rotulo#$r" "remontado $l"; done < <("$P" conferir "$S/ext/db" 2>&1)
    umount "$S/ext"
    losetup -d "$LOOP"
    LOOP=""
    umount "$S/back"
  done
}

# ------------------------------------------------------------------ 522 (P4)
# O provisionamento fino do c1, e nenhum `fsync` do motor: quem manda ao disco
# e o `syncfs` do roteiro, com o disco cheio. O modo diz como o processo que
# inseriu termina: `inserir` fecha pelo `Drop`; `inserir-e-cai` cai sem ele.
c522() {
  local rotulo="$1" modo="$2" r img="$S/back/disco.img" livre
  for r in $(seq 1 "$RODADAS"); do
    mount -t tmpfs -o size=80m tmpfs "$S/back"
    truncate -s 64M "$img"
    mkfs.ext4 -q -F -N 64 -J size=4 "$img"
    fallocate -l 64M "$img"
    LOOP="$(losetup -f --show "$img")"
    mount -t ext4 "$LOOP" "$S/ext"
    "$P" criar "$S/ext/db"
    sync
    umount "$S/ext"
    for faixa in $(dumpe2fs "$img" 2>/dev/null | sed -n 's/^ *Free blocks: //p' | tr ',' '\n' | tr -d ' '); do
      [ -z "$faixa" ] && continue
      local a="${faixa%-*}" b="${faixa#*-}"
      fallocate -p -o $((a * 4096)) -l $(((b - a + 1) * 4096)) "$img"
    done
    livre="$(df -k --output=avail "$S/back" | tail -1)"
    dd if=/dev/zero of="$S/back/enchimento" bs=1k count=$((livre - 64)) 2>/dev/null
    mount -t ext4 "$LOOP" "$S/ext"
    while IFS= read -r l; do anotar "$rotulo#$r" "$l"; done < <("$P" "$modo" "$S/ext/db" 5000 2>&1)
    while IFS= read -r l; do anotar "$rotulo#$r" "antes $l"; done < <(SO_LER=1 "$P" conferir "$S/ext/db" 2>&1 | grep '^byte52=')
    # A escrita de fundo, com o disco cheio: o que tem bloco novo nao chega.
    sync -f "$S/ext/db/pedidos.ndx" 2>/dev/null
    anotar "$rotulo#$r" "syncfs=$?"
    rm -f "$S/back/enchimento"
    umount "$S/ext"
    mount -t ext4 "$LOOP" "$S/ext"
    while IFS= read -r l; do anotar "$rotulo#$r" "remontado $l"; done < <(SO_LER=1 "$P" conferir "$S/ext/db" 2>&1)
    umount "$S/ext"
    losetup -d "$LOOP"
    LOOP=""
    umount "$S/back"
  done
}

# ------------------------------------------------------------------ 533 (C4)
# Sem provisionamento fino: aqui nada recusa. Quem decide o que chegou ao disco
# e o `queda.py`, e o resto do cache morre com o sistema de arquivos. A janela
# e `inserir` (5.000 filhas novas do cliente 2) ou `mover` (5.000 filhas do 1
# passam ao 2 no mesmo slot -- o `.reg` nem cresce).
c533() {
  local rotulo="$1" janela="$2" queda="$3" r img="$S/back/disco.img"
  for r in $(seq 1 "$RODADAS"); do
    mount -t tmpfs -o size=160m tmpfs "$S/back"
    truncate -s 128M "$img"
    mkfs.ext4 -q -F "$img"
    LOOP="$(losetup -f --show "$img")"
    mount -t ext4 "$LOOP" "$S/ext"
    while IFS= read -r l; do anotar "$rotulo#$r" "$l"; done < <("$P" criar533 "$S/ext/db" 30000 2>&1)
    sync
    while IFS= read -r l; do anotar "$rotulo#$r" "$l"; done < <("$P" janela533 "$S/ext/db" 5000 "$janela" 2>&1)
    while IFS= read -r l; do anotar "$rotulo#$r" "$l"; done < <(python3 "$AQUI/queda.py" "$S/ext/db" filhas "$queda" 2>&1)
    umount "$S/ext"
    mount -t ext4 "$LOOP" "$S/ext"
    while IFS= read -r l; do anotar "$rotulo#$r" "remontado $l"; done < <("$P" conferir533 "$S/ext/db" 2>&1)
    umount "$S/ext"
    losetup -d "$LOOP"
    LOOP=""
    umount "$S/back"
  done
}

CENARIOS="${CENARIOS:-512 509 522 533}"
if [[ " $CENARIOS " == *" 512 "* ]]; then
echo "== 512 (C2b): tmpfs de 512 KiB, segundo fecho no mesmo punho"
c2b "512-mesmo-punho" ""
echo "== 512 (C2b): tmpfs de 512 KiB, so o Drop"
c2b "512-so-o-drop" 1
fi
if [[ " $CENARIOS " == *" 509 "* ]]; then
echo "== 509 (C1): provisionamento fino, dois fechos no mesmo processo (biblioteca)"
c1 "509-biblioteca" ""
echo "== 509 (C1): provisionamento fino, o gancho do servidor (abort na recusa)"
c1 "509-servidor" 1
fi
if [[ " $CENARIOS " == *" 522 "* ]]; then
echo "== 522 (P4): provisionamento fino, syncfs com o disco cheio, fechar pelo Drop"
c522 "522-fechar" inserir
echo "== 522 (P4): o controle -- o processo cai sem Drop"
c522 "522-cai" inserir-e-cai
fi
if [[ " $CENARIOS " == *" 533 "* ]]; then
echo "== 533 (C4'): 5.000 filhas novas, o .reg chega e nada do .ndx"
c533 "533-inserir-nada" inserir nada
echo "== 533 (C4): 5.000 filhas novas, o .reg e as paginas do .ndx chegam, a pagina 0 nao"
c533 "533-inserir-paginas" inserir paginas
echo "== 533 (move): 5.000 filhas mudam de pai no mesmo slot, o .reg chega e nada do .ndx"
c533 "533-mover-nada" mover nada
fi

python3 - "$REG" "$SAIDA" "$P" "$RODADAS" <<'PY'
import datetime, json, os, platform, sys
reg, saida, binario, rodadas = sys.argv[1:5]
por = {}
for linha in open(reg, encoding="utf-8"):
    rotulo, _, texto = linha.rstrip("\n").partition("|")
    cen, _, r = rotulo.partition("#")
    por.setdefault(cen, {}).setdefault(r, []).append(texto)
json.dump({
    "medido_em": datetime.datetime.now(datetime.timezone.utc).isoformat(timespec="seconds"),
    "nucleo": platform.release(),
    "binario": os.path.basename(binario),
    "rodadas": int(rodadas),
    "cenarios": por,
}, open(saida, "w", encoding="utf-8"), ensure_ascii=False, indent=1)
print(f"gravado: {saida}")
PY
