#!/usr/bin/env bash
# Monta os pacotes do PhxZip -- `phxzip-<versao>-<plataforma>.zip` -- com o
# `phxzipcmd`, o `phxzipweb`, o manual, a licenca e o MANIFESTO.sha256 por
# dentro, e o SHA256SUMS por fora. Pedido 455, fatia Z12.
#
#   ./empacotar-phxzip.sh            as quatro plataformas do PhxSql
#   ./empacotar-phxzip.sh linux      so uma: linux, windows, arm64 ou arm32
#   ./empacotar-phxzip.sh conferir   confere o que ja esta em pacotes/phxzip/
#
# MESMO MOTOR: este script nao tem receita propria de manifesto, de zip, de
# conferencia, de versao nem de alvo. Ele carrega o `empacotar.sh` com
# `source` (o arquivo para antes do despacho dele) e chama as funcoes de la --
# `confere_versoes`, `alvo_instalado`, `ligador_musl`,
# `confere_ferramentas_windows`, `fecha` e `conferir`. A receita do manifesto
# ja existiu em quatro lugares do empacotador e os quatro divergiram
# (`bancada/pacote/provar-manifesto.py`); uma quinta copia aqui seria o mesmo
# defeito com outro nome.
#
# O que e DESTE arquivo, e por que nao cabia la:
#   * o que vai em cada pacote (dois binarios, outro manual, a licenca da fonte
#     Exo 2 que o phxzipweb embute) -- o `monta()` do PhxSql copia os tres
#     binarios dele e o config de demonstracao, e nao ha o que parametrizar
#     sem transformar o monta() numa lista de casos;
#   * plataforma que nao monta vira NAO MONTADO, com o motivo e o comando, e
#     as outras seguem -- no PhxSql a falta de alvo para tudo (`exit 1`);
#   * a conferencia da FORMA de cada binario (arquitetura pelo `file`), antes
#     do manifesto.
#
# O que cada pacote leva e como conferir: docs/MANUAL-PHXZIP.md, secao 2.

set -euo pipefail
cd "$(dirname "$0")"

# shellcheck source=empacotar.sh
source ./empacotar.sh
SAIDA=${PHXZIP_SAIDA:-pacotes/phxzip}
QUAL=${1:-tudo}

MANUAL_PHXZIP=docs/MANUAL-PHXZIP.md
FONTE_OFL=crates/phxzip-web/ui/fonte/OFL.txt

# A forma que o binario de cada plataforma tem de ter, pelo `file -b`. Pacote
# com o binario errado dentro (o de Linux no zip de ARM, por exemplo) passa no
# manifesto -- o hash e do arquivo que esta la -- e so a forma o pega.
forma_esperada() {
  case "$1" in
    linux)   echo 'ELF 64-bit .*x86-64' ;;
    windows) echo 'PE32\+ executable .*x86-64' ;;
    arm64)   echo 'ELF 64-bit .*ARM aarch64.*statically linked' ;;
    arm32)   echo 'ELF 32-bit .*ARM, EABI5.*statically linked' ;;
    *) return 1 ;;
  esac
}

nao_montado() {
  local rotulo=$1 motivo=$2 comando=$3
  local nota="$SAIDA/phxzip-$VERSAO-$rotulo.NAO-MONTADO.txt"
  printf 'phxzip-%s-%s: NAO MONTADO\n\nmotivo: %s\n\npara montar:\n%s\n' \
    "$VERSAO" "$rotulo" "$motivo" "$comando" > "$nota"
  rm -f "$SAIDA/phxzip-$VERSAO-$rotulo.zip"
  echo "   NAO MONTADO: $motivo"
  echo "   (ficou em $nota)"
}

licenca() {
  local dir=$1 spdx
  spdx=$(grep -m1 '^license' Cargo.toml | cut -d'"' -f2)
  cat > "$dir/LICENCA.txt" <<TXT
PhxZip $VERSAO -- licenca

Licenca do PhxZip, do PhxZipCmd e do PhxZipWeb (o campo \`license\` do
Cargo.toml do repositorio, lido por quem montou este pacote):

    $spdx

O repositorio ainda nao traz o texto das licencas num arquivo LICENSE.
Escolher e colar esse texto e decisao do dono do projeto, e nao do
empacotador (docs/EMPACOTAMENTO.md, secao 4). Enquanto isso, vale o
identificador SPDX acima.

A fonte Exo 2, que o phxzipweb embute para a tela abrir sem rede, e da SIL
Open Font License 1.1. O texto inteiro vai em fonte-exo2-OFL.txt, como a
licenca pede.
TXT
  cp "$FONTE_OFL" "$dir/fonte-exo2-OFL.txt"
}

comece_aqui() {
  local dir=$1 rotulo=$2 sufixo=$3
  local pre="./" conferir_txt
  if [ "$rotulo" = "windows" ]; then
    pre=""
    conferir_txt="       Get-FileHash .\\phxzipcmd.exe -Algorithm SHA256     (PowerShell)

e compare com a linha do phxzipcmd.exe no MANIFESTO.sha256; o mesmo para cada
arquivo."
  else
    conferir_txt="       sha256sum -c MANIFESTO.sha256

Ele responde OK por arquivo; qualquer FAILED quer dizer pacote alterado."
  fi
  cat > "$dir/COMECE-AQUI.txt" <<TXT
================================================================================
PHXZIP $VERSAO -- $rotulo
================================================================================

Os dois programas:

       ${pre}phxzipcmd$sufixo   no terminal: listar, testar, compactar, extrair (.7z)
       ${pre}phxzipweb$sufixo   no navegador, so nesta maquina (127.0.0.1)

Para ver a tela:

       ${pre}phxzipweb$sufixo
       e abra o endereco que ele imprimir

O manual inteiro -- o que cada um recusa e por que, os tetos, a bancada contra
o 7-Zip -- esta em MANUAL-PHXZIP.md, nesta pasta.

ANTES DE RODAR: CONFIRA O PACOTE
--------------------------------------------------------------------------------
O MANIFESTO.sha256 tem o SHA-256 de cada arquivo desta pasta:

$conferir_txt

Este pacote nao traz conferidor proprio: o conferir-pacote mora no phxsql, o
programa de linha de comando do PhxSql, e serve igual para esta pasta
(phxsql conferir-pacote <esta pasta>).
================================================================================
TXT
}

monta_phxzip() {
  local alvo=$1 rotulo=$2 sufixo=$3
  local nome="phxzip-$VERSAO-$rotulo"
  local dir="$SAIDA/$nome"
  echo "== $rotulo ($alvo)"

  if ! alvo_instalado "$alvo"; then
    nao_montado "$rotulo" "o alvo $alvo nao esta instalado nesta maquina" \
      "    rustup target add $alvo
    ./empacotar-phxzip.sh $rotulo"
    return 0
  fi
  if [ "$rotulo" = "windows" ] && ! confere_ferramentas_windows; then
    nao_montado "$rotulo" "falta o ligador $LIGADOR_WINDOWS (ou o alvo)" \
      "    sudo apt install mingw-w64      # ou o equivalente da distro
    ./empacotar-phxzip.sh windows"
    return 0
  fi
  case "$alvo" in *-musl*) ligador_musl "$alvo" ;; esac

  if ! cargo build --release --offline --target "$alvo" \
      -p phxzip-cmd --bin phxzipcmd -p phxzip-web --bin phxzipweb; then
    nao_montado "$rotulo" "o cargo build para $alvo falhou (ver a saida acima)" \
      "    cargo build --release --offline --target $alvo -p phxzip-cmd -p phxzip-web
    ./empacotar-phxzip.sh $rotulo"
    return 0
  fi

  rm -rf "$dir"; mkdir -p "$dir"
  local b padrao
  padrao=$(forma_esperada "$rotulo")
  for b in phxzipcmd phxzipweb; do
    cp "target/$alvo/release/$b$sufixo" "$dir/"
    file -b "$dir/$b$sufixo" | grep -qE "$padrao" || {
      echo "   o $b$sufixo nao tem a forma de $rotulo: $(file -b "$dir/$b$sufixo")"
      exit 1
    }
  done
  cp "$MANUAL_PHXZIP" "$dir/MANUAL-PHXZIP.md"
  licenca "$dir"
  comece_aqui "$dir" "$rotulo" "$sufixo"
  rm -f "$SAIDA/$nome.NAO-MONTADO.txt"
  fecha "$nome"

  # O disco desta maquina e apertado, e o target de um alvo cruzado so serve
  # para remontar o mesmo pacote: o binario ja esta no zip.
  rm -rf "target/${alvo:?}"
}

# A conferencia do PhxZip: a FORMA de cada binario, a lista do que nao foi
# montado, e depois a MESMA conferencia do PhxSql -- SHA256SUMS por fora e o
# `phxsql conferir-pacote` por dentro, que reprova byte trocado, arquivo a
# mais e arquivo faltando.
conferir_phxzip() {
  local falhas=0 z nome rotulo padrao tmp b forma n
  shopt -s nullglob
  local zips=("$SAIDA"/phxzip-*.zip) faltas=("$SAIDA"/*.NAO-MONTADO.txt)
  shopt -u nullglob

  tmp=$(mktemp -d)
  for z in "${zips[@]}"; do
    nome=$(basename "$z" .zip)
    rotulo=${nome##*-}
    echo "== forma: $nome"
    padrao=$(forma_esperada "$rotulo") || { echo "   plataforma desconhecida: $rotulo"; falhas=$((falhas + 1)); continue; }
    if ! unzip -q "$z" -d "$tmp"; then
      echo "   o zip nao abre inteiro"
      falhas=$((falhas + 1))
      continue
    fi
    n=0
    for b in "$tmp/$nome"/phxzipcmd* "$tmp/$nome"/phxzipweb*; do
      [ -f "$b" ] || continue
      n=$((n + 1))
      forma=$(file -b "$b")
      if echo "$forma" | grep -qE "$padrao"; then
        echo "   ok  $(basename "$b"): $forma"
      else
        echo "   ERRADO  $(basename "$b"): $forma"
        falhas=$((falhas + 1))
      fi
    done
    [ "$n" -eq 2 ] || { echo "   esperava 2 binarios, achou $n"; falhas=$((falhas + 1)); }
    rm -rf "${tmp:?}/$nome"
  done
  rm -rf "$tmp"

  for f in "${faltas[@]}"; do
    echo
    echo "== $(basename "$f" .txt)"
    sed 's/^/   /' "$f"
  done

  echo
  # O `conferir` do PhxSql sai com `exit` quando reprova; no subshell ele
  # reprova so a si mesmo, e a falha de forma acima continua no veredito --
  # zip adulterado que nem abre nao pode esconder o SHA256SUMS que o acusa.
  if ! ( conferir ); then falhas=$((falhas + 1)); fi
  [ ${#faltas[@]} -eq 0 ] || echo "${#faltas[@]} plataforma(s) NAO MONTADA(S) -- ver acima."
  [ $falhas -eq 0 ] || { echo "os pacotes do PhxZip NAO conferem ($falhas falha(s))."; exit 1; }
}

monta_por_rotulo() {
  case "$1" in
    linux)   monta_phxzip "$ALVO_LINUX" linux "" ;;
    windows) monta_phxzip "$ALVO_WINDOWS" windows .exe ;;
    arm64)   monta_phxzip "$ALVO_ARM64" arm64 "" ;;
    arm32)   monta_phxzip "$ALVO_ARM32" arm32 "" ;;
  esac
}

# Os numeros do manual saem do `bancada/phxzip/manual.py`; manual com numero
# velho nao viaja. Guardado por `command -v` como o `confere_versoes` faz, e
# ruidoso quando pula.
confere_manual() {
  echo "== numeros do $MANUAL_PHXZIP"
  if command -v python3 >/dev/null 2>&1; then
    python3 bancada/phxzip/manual.py --catraca ||
      { echo "   o manual tem numero velho -- nada foi empacotado."; exit 4; }
  else
    echo "   PULADO: sem python3, os numeros do manual nao foram conferidos"
  fi
}

mkdir -p "$SAIDA"
case "$QUAL" in
  conferir) conferir_phxzip; exit 0 ;;
  linux|windows|arm64|arm32)
    confere_versoes; confere_manual; monta_por_rotulo "$QUAL" ;;
  tudo)
    confere_versoes; confere_manual
    for r in linux windows arm64 arm32; do monta_por_rotulo "$r"; done
    ;;
  *) echo "uso: $0 [linux|windows|arm64|arm32|tudo|conferir]" >&2; exit 2 ;;
esac

# A lista de fora, como a do PhxSql: o hash dos zips, para conferir o download
# antes de abrir.
( cd "$SAIDA" && rm -f SHA256SUMS
  shopt -s nullglob; zs=(./*.zip)
  [ ${#zs[@]} -eq 0 ] || sha256sum "${zs[@]}" | sed 's|\./||' > SHA256SUMS )

echo
ls -lh "$SAIDA"/*.zip 2>/dev/null || echo "nenhum zip em $SAIDA/"
shopt -s nullglob
faltas=("$SAIDA"/*.NAO-MONTADO.txt)
shopt -u nullglob
if [ ${#faltas[@]} -gt 0 ]; then
  echo
  echo "NAO MONTADO (${#faltas[@]}):"
  for f in "${faltas[@]}"; do echo "   $(basename "$f" .NAO-MONTADO.txt): $(sed -n 3p "$f")"; done
  exit 3
fi
