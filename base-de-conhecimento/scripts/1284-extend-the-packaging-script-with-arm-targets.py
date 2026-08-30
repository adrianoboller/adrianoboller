# Extend the packaging script with ARM targets
# 30/08 15:32

p='empacotar.sh'
s=open(p,encoding='utf-8').read()

velho='''ALVO_LINUX=x86_64-unknown-linux-gnu
ALVO_WINDOWS=x86_64-pc-windows-gnu
LIGADOR_WINDOWS=x86_64-w64-mingw32-gcc'''
novo='''ALVO_LINUX=x86_64-unknown-linux-gnu
ALVO_WINDOWS=x86_64-pc-windows-gnu
LIGADOR_WINDOWS=x86_64-w64-mingw32-gcc

# Os dois alvos de placa pequena. Sao `musl` porque o binario sai ESTATICO: um
# arquivo so, sem carregador dinamico e sem depender da libc que a distribuicao
# da placa trouxe -- que e o que faz o mesmo arquivo servir num Raspberry, num
# roteador e num contentor `scratch`. Zero dependencia externa e o que torna
# isso possivel; com uma crate de C no meio, cada uma delas teria de compilar
# cruzado tambem.
ALVO_ARM64=aarch64-unknown-linux-musl
ALVO_ARM32=armv7-unknown-linux-musleabihf'''
assert s.count(velho)==1
s=s.replace(velho,novo)

velho2='''monta() {
  local alvo=$1 rotulo=$2 sufixo=$3
  local nome="phxsql-$VERSAO-$rotulo"
  local dir="$SAIDA/$nome"

  echo "== $rotulo ($alvo)"
  if [ "$rotulo" = "windows" ]; then confere_ferramentas_windows; fi
  cargo build --release --offline --workspace --target "$alvo"
'''
novo2='''monta() {
  local alvo=$1 rotulo=$2 sufixo=$3 sem_odbc=${4:-}
  local nome="phxsql-$VERSAO-$rotulo"
  local dir="$SAIDA/$nome"

  echo "== $rotulo ($alvo)"
  if [ "$rotulo" = "windows" ]; then confere_ferramentas_windows; fi
  if ! rustup target list --installed 2>/dev/null | grep -qx "$alvo"; then
    echo "   FALTA o alvo $alvo:"
    echo "       rustup target add $alvo"
    exit 1
  fi
  if [ -n "$sem_odbc" ]; then
    # `musl` nao produz `cdylib`, entao o driver ODBC nao cabe neste pacote --
    # e nao faz falta: o driver e do lado CLIENTE, na maquina que roda a
    # ferramenta de relatorio, nao na placa que guarda o dado. Quem precisar de
    # ODBC em ARM compila para o alvo `gnu` da mesma arquitetura.
    #
    # O ligador e o `rust-lld` que ja vem com a ferramenta: sem dependencia de
    # C, nao ha o que um `gcc` cruzado teria de resolver.
    local var="CARGO_TARGET_$(echo "$alvo" | tr 'a-z-' 'A-Z_')_LINKER"
    export "$var=rust-lld"
    cargo build --release --offline --target "$alvo" \\
      -p phxsql-server --bin phxsqld \\
      -p phxsql-cli --bin phxsql \\
      -p phxsql-cmd --bin phxsqlcmd
  else
    cargo build --release --offline --workspace --target "$alvo"
  fi
'''
assert s.count(velho2)==1
s=s.replace(velho2,novo2)

velho3='''  if [ "$rotulo" = "windows" ]; then
    cp "target/$alvo/release/phxsql_odbc.dll" "$dir/"
  else
    cp "target/$alvo/release/libphxsql_odbc.so" "$dir/"
  fi'''
novo3='''  if [ -n "$sem_odbc" ]; then
    : # ver o comentario do `sem_odbc` acima
  elif [ "$rotulo" = "windows" ]; then
    cp "target/$alvo/release/phxsql_odbc.dll" "$dir/"
  else
    cp "target/$alvo/release/libphxsql_odbc.so" "$dir/"
  fi'''
assert s.count(velho3)==1
s=s.replace(velho3,novo3)

velho4='''  linux)   confere_versoes; monta "$ALVO_LINUX" linux "" ;;
  windows) confere_versoes; monta "$ALVO_WINDOWS" windows .exe ;;
  fontes)  confere_versoes; fontes ;;
  tudo)
    confere_versoes
    monta "$ALVO_LINUX" linux ""
    monta "$ALVO_WINDOWS" windows .exe
    fontes
    ;;
  *) echo "uso: $0 [linux|windows|fontes|tudo|conferir]" >&2; exit 2 ;;'''
novo4='''  linux)   confere_versoes; monta "$ALVO_LINUX" linux "" ;;
  windows) confere_versoes; monta "$ALVO_WINDOWS" windows .exe ;;
  arm64)   confere_versoes; monta "$ALVO_ARM64" arm64 "" sem_odbc ;;
  arm32)   confere_versoes; monta "$ALVO_ARM32" arm32 "" sem_odbc ;;
  fontes)  confere_versoes; fontes ;;
  tudo)
    confere_versoes
    monta "$ALVO_LINUX" linux ""
    monta "$ALVO_WINDOWS" windows .exe
    monta "$ALVO_ARM64" arm64 "" sem_odbc
    monta "$ALVO_ARM32" arm32 "" sem_odbc
    fontes
    ;;
  *) echo "uso: $0 [linux|windows|arm64|arm32|fontes|tudo|conferir]" >&2; exit 2 ;;'''
assert s.count(velho4)==1
s=s.replace(velho4,novo4)
open(p,'w',encoding='utf-8').write(s)
print("empacotar.sh: alvos arm64 e arm32 acrescentados")
