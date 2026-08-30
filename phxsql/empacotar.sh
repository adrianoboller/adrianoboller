#!/usr/bin/env bash
# Monta os pacotes de download -- fontes, Linux e Windows --, cada um com o
# manual junto e com um manifesto SHA-256 que se confere.
#
# Existe por um motivo simples: os pacotes das rodadas anteriores foram feitos
# a mao. Pacote feito a mao e pacote que ninguem consegue refazer igual -- e a
# primeira coisa que alguem pede quando o binario da problema e "como voce
# gerou isso?".
#
#   ./empacotar.sh            monta os tres pacotes em pacotes/
#   ./empacotar.sh linux      so o de Linux
#   ./empacotar.sh windows    so o de Windows
#   ./empacotar.sh fontes     so o dos fontes
#   ./empacotar.sh conferir   desempacota o que ja esta em pacotes/ e confere
#
# O que cada pacote leva, o que ele NAO leva e como quem baixou confere esta
# em docs/EMPACOTAMENTO.md.

set -euo pipefail
cd "$(dirname "$0")"

VERSAO=$(grep -m1 '^version' Cargo.toml | cut -d'"' -f2)
SAIDA=pacotes
QUAL=${1:-tudo}

ALVO_LINUX=x86_64-unknown-linux-gnu
ALVO_WINDOWS=x86_64-pc-windows-gnu
LIGADOR_WINDOWS=x86_64-w64-mingw32-gcc

# Os dois alvos de placa pequena. Sao `musl` porque o binario sai ESTATICO: um
# arquivo so, sem carregador dinamico e sem depender da libc que a distribuicao
# da placa trouxe -- que e o que faz o mesmo arquivo servir num Raspberry, num
# roteador e num contentor `scratch`. Zero dependencia externa e o que torna
# isso possivel; com uma crate de C no meio, cada uma delas teria de compilar
# cruzado tambem.
ALVO_ARM64=aarch64-unknown-linux-musl
ALVO_ARM32=armv7-unknown-linux-musleabihf

MANIFESTO=MANIFESTO.sha256

# Contado, nunca digitado: o numero de crates aparece na conferencia de versao
# e no COMECE-AQUI dos fontes, e ja mudou tres vezes neste projeto.
CRATES=$(ls -d crates/*/ | wc -l | tr -d ' ')
FERRUGEM=$(grep -m1 '^rust-version' Cargo.toml | cut -d'"' -f2)

# A interface e os exemplos de tela entram por include_str!, entao o pacote e
# so os binarios mais a documentacao. Nada de arquivo solto para se perder.
DOCS=(MANUAL.txt README.md CHANGELOG.md)

# Os dois documentos que o COMECE-AQUI cita pelo nome. Documento citado tem de
# viajar junto: quem baixou o zip nao tem o repositorio para ir buscar.
DOCS_DE_APOIO=(docs/ODBC.md docs/CONSOLE.md)

mkdir -p "$SAIDA"

# ---------------------------------------------------------------------------
# Um numero de versao, quatro lugares
#
# Numero digitado a mao envelhece calado -- o selo da capa do dossie passou
# quatro lancamentos dizendo 0.11.0 e ninguem viu. Aqui a versao aparece no
# Cargo.toml, no Cargo.lock, no cabecalho do MANUAL e no titulo mais novo do
# CHANGELOG. Se os quatro nao disserem a mesma coisa, o pacote nao sai: um zip
# chamado 0.18.0 com um manual de 0.17.0 e pior que zip nenhum, porque quem
# baixou vai acreditar no manual.
confere_versoes() {
  local erro=0
  echo "== versao $VERSAO"

  local c
  for c in crates/*/Cargo.toml; do
    # Um `version = "0.17.0"` solto no crate publicaria binario com a versao
    # de ontem sem ninguem notar.
    grep -q '^version.workspace = true' "$c" ||
      { echo "   $c nao herda version.workspace"; erro=1; }
  done

  local no_lock
  no_lock=$(grep -c "^version = \"$VERSAO\"\$" Cargo.lock | tr -d ' ' || true)
  [ "$no_lock" = "$CRATES" ] ||
    { echo "   Cargo.lock traz $no_lock crates em $VERSAO, e o workspace tem $CRATES"; erro=1; }

  local no_manual
  no_manual=$(grep -m1 -oE 'versao [0-9]+\.[0-9]+\.[0-9]+' MANUAL.txt | cut -d' ' -f2 || true)
  [ "$no_manual" = "$VERSAO" ] ||
    { echo "   o cabecalho do MANUAL.txt diz '${no_manual:-nada}'"; erro=1; }

  local no_changelog
  no_changelog=$(grep -m1 -oE '^## [0-9]+\.[0-9]+\.[0-9]+' CHANGELOG.md | cut -d' ' -f2 || true)
  [ "$no_changelog" = "$VERSAO" ] ||
    { echo "   o titulo lancado mais novo do CHANGELOG.md e '${no_changelog:-nenhum}'"; erro=1; }

  # O quinto lugar: o nome do zip escrito a mao dentro de um documento. A
  # secao 4 do MANUAL manda `unzip phxsql-<versao>-linux.zip`, e um numero
  # digitado ali envelhece tao calado quanto o selo da capa do dossie -- com o
  # agravante de mandar o leitor procurar um arquivo que nao existe.
  local fora
  fora=$(grep -ohE 'phxsql-[0-9]+\.[0-9]+\.[0-9]+' MANUAL.txt README.md docs/*.md |
    grep -v "^phxsql-$VERSAO\$" | sort -u || true)
  [ -z "$fora" ] ||
    { echo "   nome de arquivo com versao velha: $(echo "$fora" | tr '\n' ' ')"; erro=1; }

  [ $erro -eq 0 ] || { echo; echo "as versoes nao batem -- nada foi empacotado."; exit 4; }
}

# O config de demonstracao pede o hash da senha ao proprio phxsqld, e nao a uma
# constante colada aqui: nao existe uma segunda implementacao de senha neste
# projeto. So que o monta() compila com --target, que grava em
# target/<alvo>/release, e o binario do HOSPEDEIRO -- o unico que roda aqui --
# fica em target/release. Num checkout limpo ele nao existia, e o empacotador
# morria nesta linha; so nao morria na maquina de quem tinha rodado
# `cargo build --release` antes. Pacote so se confia se nasce de arvore limpa.
garante_host() {
  [ -x target/release/phxsqld ] && return 0
  echo "== phxsqld do hospedeiro (so para gerar o hash da senha de demonstracao)"
  cargo build --release --offline -p phxsql-server --bin phxsqld
}

# Alvo e ligador antes de compilar, com o comando exato de quem nao os tem.
# Falhar no meio de um `cargo build` de tres minutos com "linker not found" nao
# diz a ninguem o que instalar.
confere_ferramentas_windows() {
  local falta=0
  if ! rustup target list --installed 2>/dev/null | grep -qx "$ALVO_WINDOWS"; then
    echo "   FALTA o alvo $ALVO_WINDOWS:"
    echo "       rustup target add $ALVO_WINDOWS"
    falta=1
  fi
  if ! command -v "$LIGADOR_WINDOWS" >/dev/null 2>&1; then
    echo "   FALTA o ligador $LIGADOR_WINDOWS:"
    echo "       Debian/Ubuntu   sudo apt install mingw-w64"
    echo "       Fedora          sudo dnf install mingw64-gcc"
    echo "       Arch            sudo pacman -S mingw-w64-gcc"
    falta=1
  fi
  [ $falta -eq 0 ] && return 0
  echo
  echo "   O pacote de Windows nao sai desta maquina sem os dois. Os de Linux"
  echo "   e de fontes saem: ./empacotar.sh linux e ./empacotar.sh fontes."
  return 3
}

# ---------------------------------------------------------------------------
# O manifesto, e por que ele tem o formato do sha256sum
#
# Um pacote de distribuicao merece a mesma disciplina que o backup de dados ja
# tem: copia mais SHA-256 de cada arquivo, e alguem que leia tudo de volta e
# confira. Copia que ninguem consegue conferir e esperanca, nao copia.
#
# O formato e o do `sha256sum` -- `<hex>  <caminho>` -- de proposito, para o
# pacote ter DOIS conferidores independentes:
#
#   phxsql conferir-pacote    viaja dentro do proprio pacote, roda igual no
#                             Windows e usa o SHA-256 daqui, que e conferido
#                             contra os vetores do FIPS 180-4;
#   sha256sum -c              para quem preferir nao rodar o binario que esta
#                             justamente conferindo.
#
# O manifesto lista o caminho relativo com barra normal, ordenado, e nao lista
# a si mesmo.
manifesto() {
  local dir=$1
  ( cd "$dir"
    : > "$MANIFESTO"
    find . -type f ! -name "$MANIFESTO" | sed 's|^\./||' | LC_ALL=C sort |
      while IFS= read -r f; do sha256sum "$f"; done >> "$MANIFESTO"
  )
  echo "   $(wc -l < "$dir/$MANIFESTO") arquivos no $MANIFESTO"
}

# Fecha o pacote: manifesto, zip, e o hash do proprio zip na lista de fora.
fecha() {
  local nome=$1
  local dir="$SAIDA/$nome"
  manifesto "$dir"
  ( cd "$SAIDA" && rm -f "$nome.zip" && zip -qr "$nome.zip" "$nome" )
  rm -rf "$dir"
  echo "   $SAIDA/$nome.zip"
}

monta() {
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
    cargo build --release --offline --target "$alvo" \
      -p phxsql-server --bin phxsqld \
      -p phxsql-cli --bin phxsql \
      -p phxsql-cmd --bin phxsqlcmd
  else
    cargo build --release --offline --workspace --target "$alvo"
  fi

  rm -rf "$dir"; mkdir -p "$dir"
  cp "target/$alvo/release/phxsqld$sufixo" "$dir/"
  cp "target/$alvo/release/phxsql$sufixo"  "$dir/"
  # O console entrou na 0.18.0 e quase ficou de fora do pacote: o empacotador
  # nao sabia dele. Binario novo se acrescenta AQUI, ou o download mente.
  cp "target/$alvo/release/phxsqlcmd$sufixo" "$dir/"
  # O driver ODBC e biblioteca, nao executavel: o nome muda por plataforma.
  # Como instalar e registrar esta em docs/ODBC.md, que vai junto.
  if [ -n "$sem_odbc" ]; then
    : # ver o comentario do `sem_odbc` acima
  elif [ "$rotulo" = "windows" ]; then
    cp "target/$alvo/release/phxsql_odbc.dll" "$dir/"
  else
    cp "target/$alvo/release/libphxsql_odbc.so" "$dir/"
  fi
  cp "${DOCS[@]}" "$dir/"
  mkdir -p "$dir/docs"
  cp "${DOCS_DE_APOIO[@]}" "$dir/docs/"
  # Os tres modelos de config -- isolado, source e replica. Sem eles, quem
  # baixou o binario so tem o de demonstracao, que escuta em 127.0.0.1 e nao
  # serve para servidor nenhum.
  mkdir -p "$dir/exemplos"
  cp exemplos/Config_exemplo_*.json "$dir/exemplos/"

  demonstracao "$dir" "$rotulo" "$sufixo"
  fecha "$nome"
}

# Um ambiente de teste que sobe com um comando so.
#
# Existe porque o `--exemplo 1` -- que e o config de PRODUCAO -- nasce com a
# web DESLIGADA e com senhas que quem baixa nao conhece. As duas coisas estao
# certas para um servidor de verdade e erradas para quem quer so ver a tela.
#
# O que este config faz de diferente, e por que e seguro mesmo assim:
#
#   * escuta em 127.0.0.1, e nao em 0.0.0.0 -- so alcanca quem esta NA
#     maquina. Um pacote de teste que abre porta para a rede seria um
#     presente para quem estiver do outro lado dela;
#   * a senha esta escrita no COMECE-AQUI.txt, em letras grandes, junto com
#     o aviso de que isto nao vai para producao;
#   * o hash sai do proprio `phxsqld --senha`, e nao de uma constante colada
#     aqui: nao existe uma segunda implementacao de senha neste projeto.
demonstracao() {
  local dir=$1 rotulo=$2 sufixo=$3
  local hash comandos
  garante_host
  hash=$(echo "demo" | ./target/release/phxsqld --senha | sed 's/.*: "//;s/"//')

  # A quantidade de comandos do phxsql sai da PROPRIA ajuda do binario que
  # esta sendo empacotado. Estava escrita a mao no COMECE-AQUI: dizia 10
  # quando eram 13, e ninguem conferiu porque quem le o texto nao conta a
  # lista. Numero digitado a mao envelhece calado, inclusive num arquivo-leia.
  cargo build --release --offline -p phxsql-cli --bin phxsql
  comandos=$(./target/release/phxsql | grep -cE '^  phxsql ')

  mkdir -p "$dir/demonstracao"
  # O campo e "sessao_minutos". Ele ja saiu daqui escrito "sessao_min", que o
  # servidor recusa com um AVISO e ignora -- e como o padrao tambem e 60, a
  # tela ficava igual e ninguem via. Configuracao que nao e lida mente.
  cat > "$dir/demonstracao/config.json" <<JSON
{
  "_comentario": "AMBIENTE DE DEMONSTRACAO -- nao use em producao. Escuta so em 127.0.0.1; a senha esta no COMECE-AQUI.txt. Para um servidor de verdade, gere o seu com: phxsqld --exemplo 1 > config.json",

  "bind": "127.0.0.1:5000",
  "base": "dados",
  "token": "demo",

  "web": { "ligado": true, "bind": "127.0.0.1:8080", "sessao_minutos": 60 },

  "usuarios": [
    {
      "login": "adm",
      "nome": "Administrador da demonstracao",
      "id": 1,
      "senha_hash": "$hash",
      "supervisor": true
    }
  ]
}
JSON

  if [ "$rotulo" = "windows" ]; then
    cat > "$dir/COMECE-AQUI.txt" <<TXT
================================================================================
PHXSQL $VERSAO -- COMO VER A TELA EM UM MINUTO                          Windows
================================================================================

1. Abra o Prompt de Comando NESTA PASTA e rode:

       cd demonstracao
       ..\\phxsqld.exe

2. Abra o navegador em:

       http://127.0.0.1:8080

3. Entre com:

       usuario  adm
       senha    demo
       token    demo

Pronto. O banco nasce vazio: use o botao "Bancos" para criar o primeiro, e
"Tabelas" para criar uma tabela. O menu Ajuda explica cada tela.

Para parar: Ctrl+C na janela do prompt.

--------------------------------------------------------------------------------
ANTES DE RODAR: CONFIRA O PACOTE
--------------------------------------------------------------------------------
O MANIFESTO.sha256 tem o SHA-256 de cada arquivo desta pasta. Conferir e um
comando:

       phxsql.exe conferir-pacote

Ele responde "INTEGRO" ou aponta o arquivo que mudou, o que falta e o que veio
a mais. Se preferir nao rodar o binario que esta justamente conferindo, o
Windows tem um segundo caminho, pelo PowerShell:

       Get-FileHash .\\phxsqld.exe -Algorithm SHA256

e compare com a linha do phxsqld.exe no MANIFESTO.sha256.

--------------------------------------------------------------------------------
ISTO E UMA DEMONSTRACAO, E NAO UM SERVIDOR DE PRODUCAO
--------------------------------------------------------------------------------
Ela escuta so em 127.0.0.1 -- so alcanca quem esta NESTE computador --, e a
senha esta escrita aqui em cima. Para um servidor de verdade:

       phxsqld.exe --exemplo 1 > config.json      gera o modelo comentado
       phxsqld.exe --senha                        gera o hash de uma senha

e leia a secao 7 do MANUAL.txt, que explica bind, token, usuarios e permissoes.
A senha NUNCA fica em texto puro no config.json -- so o hash. Os tres modelos
tambem estao prontos em exemplos\\: 01 isolado, 02 source, 03 replica.

Os tres programas:

       phxsqld.exe    o servidor
       phxsql.exe     a linha de comando ($comandos comandos; rode sem argumentos)
       phxsqlcmd.exe  o console interativo: conecta no servidor e /help
                      lista todos os comandos, /help <comando> detalha um
                      (docs\\CONSOLE.md)

E, para programas de terceiros (Excel, Access, e o que mais fale ODBC):

       phxsql_odbc.dll   o driver ODBC. Registro e connection string em
                         docs\\ODBC.md, secao Windows
================================================================================
TXT
  else
    cat > "$dir/COMECE-AQUI.txt" <<TXT
================================================================================
PHXSQL $VERSAO -- COMO VER A TELA EM UM MINUTO                            Linux
================================================================================

1. No terminal, NESTA PASTA:

       chmod +x phxsqld phxsql phxsqlcmd
       cd demonstracao
       ../phxsqld

2. Abra o navegador em:

       http://127.0.0.1:8080

3. Entre com:

       usuario  adm
       senha    demo
       token    demo

Pronto. O banco nasce vazio: use o botao "Bancos" para criar o primeiro, e
"Tabelas" para criar uma tabela. O menu Ajuda explica cada tela.

Para parar: Ctrl+C no terminal.

--------------------------------------------------------------------------------
ANTES DE RODAR: CONFIRA O PACOTE
--------------------------------------------------------------------------------
O MANIFESTO.sha256 tem o SHA-256 de cada arquivo desta pasta. Conferir e um
comando:

       chmod +x phxsql && ./phxsql conferir-pacote

Ele responde "INTEGRO" ou aponta o arquivo que mudou, o que falta e o que veio
a mais. E o manifesto esta no formato do sha256sum, entao ha um segundo
caminho, que nao depende de rodar nada deste pacote:

       sha256sum -c MANIFESTO.sha256

--------------------------------------------------------------------------------
ISTO E UMA DEMONSTRACAO, E NAO UM SERVIDOR DE PRODUCAO
--------------------------------------------------------------------------------
Ela escuta so em 127.0.0.1 -- so alcanca quem esta NESTA maquina --, e a senha
esta escrita aqui em cima. Para um servidor de verdade:

       ./phxsqld --exemplo 1 > config.json      gera o modelo comentado
       ./phxsqld --senha                        gera o hash de uma senha

e leia a secao 7 do MANUAL.txt, que explica bind, token, usuarios e permissoes.
A senha NUNCA fica em texto puro no config.json -- so o hash. Os tres modelos
tambem estao prontos em exemplos/: 01 isolado, 02 source, 03 replica.

Os tres programas:

       phxsqld    o servidor
       phxsql     a linha de comando ($comandos comandos; rode sem argumentos)
       phxsqlcmd  o console interativo: conecta no servidor e /help lista
                  todos os comandos, /help <comando> detalha um
                  (docs/CONSOLE.md)

E, para programas de terceiros (via unixODBC):

       libphxsql_odbc.so   o driver ODBC. Registro e connection string em
                           docs/ODBC.md, secao unixODBC
================================================================================
TXT
  fi
}

fontes() {
  local nome="phxsql-$VERSAO-fontes"
  local dir="$SAIDA/$nome"
  echo "== fontes"

  # Rodado de DENTRO do zip de fontes ja extraido nao ha .git, e o erro do git
  # sai embaralhado com a mensagem de arvore suja. Quem esta ali quer os
  # binarios, e esses saem: dizer isso vale mais que repetir o erro do git.
  if ! git rev-parse --is-inside-work-tree >/dev/null 2>&1; then
    echo "   isto nao e um clone do repositorio, e o pacote de fontes sai do"
    echo "   historico (git archive). De um diretorio extraido saem os outros"
    echo "   dois: ./empacotar.sh linux e ./empacotar.sh windows."
    exit 7
  fi

  # O `git archive` le o HEAD, e nao a arvore de trabalho: um pacote de fontes
  # feito com mudanca por commitar sai DIFERENTE do que o autor esta vendo, e
  # ninguem percebe -- nem ele, nem quem baixou.
  if ! git diff --quiet HEAD -- . || [ -n "$(git ls-files --others --exclude-standard .)" ]; then
    echo "   a arvore de trabalho difere do HEAD. O pacote de fontes sai do"
    echo "   HEAD ($(git rev-parse --short HEAD)), entao o que nao esta commitado"
    echo "   NAO vai junto. Commite antes, ou aceite empacotar o HEAD:"
    echo "       PHXSQL_EMPACOTAR_SUJO=1 ./empacotar.sh fontes"
    [ "${PHXSQL_EMPACOTAR_SUJO:-}" = "1" ] || exit 5
  fi

  rm -rf "$dir"; mkdir -p "$dir"
  # git archive respeita o .gitignore de graca: os 2,4 GB da bancada ficam de
  # fora sem ninguem precisar lembrar. E, rodado daqui de dentro, ele recorta
  # o subdiretorio -- o zip nasce com o Cargo.toml na raiz, e nao debaixo de
  # phxsql/. A conferencia logo abaixo existe porque isso e sutil: se o git
  # mudar de ideia, o pacote deixa de compilar e o empacotador tem de ser o
  # primeiro a saber.
  git archive --format=tar HEAD | tar -x -C "$dir"
  [ -f "$dir/Cargo.toml" ] ||
    { echo "   o archive nao trouxe o Cargo.toml na raiz -- pacote nao compilaria."; exit 6; }

  cat > "$dir/COMECE-AQUI.txt" <<TXT
================================================================================
PHXSQL $VERSAO -- OS FONTES                                     HEAD $(git rev-parse --short HEAD)
================================================================================

Este e o codigo inteiro: os $CRATES crates, a interface web, os documentos, a
bancada de medicao e as baterias de teste. Os binarios prontos estao nos
outros dois zips (...-linux.zip e ...-windows.zip).

COMPILAR
--------------------------------------------------------------------------------
Precisa de Rust $FERRUGEM ou mais novo, e de mais nada:

       cargo build --offline --release

O --offline nao e teimosia: este projeto NAO TEM NENHUMA DEPENDENCIA EXTERNA.
O Cargo.lock lista $CRATES pacotes, e os $CRATES sao deste repositorio -- nada de
registro, nada de git, nada para baixar. Compila numa maquina sem rede.

Os binarios saem em target/release/:

       phxsqld      o servidor
       phxsql       a linha de comando
       phxsqlcmd    o console interativo
       libphxsql_odbc.so / phxsql_odbc.dll     o driver ODBC

Para o executavel de Windows a partir do Linux:

       rustup target add x86_64-pc-windows-gnu
       sudo apt install mingw-w64            # ou o equivalente da sua distro
       cargo build --offline --release --target x86_64-pc-windows-gnu

E, para refazer os pacotes de binario a partir daqui:

       ./empacotar.sh linux
       ./empacotar.sh windows

O de fontes so sai de um clone do repositorio: ele nasce do historico do git,
que este zip nao carrega.

CONFERIR ESTE PACOTE
--------------------------------------------------------------------------------
O MANIFESTO.sha256 tem o SHA-256 de cada arquivo:

       sha256sum -c MANIFESTO.sha256

Depois de compilar, ha um segundo conferidor, que nao depende do sha256sum e
roda igual no Windows:

       ./target/release/phxsql conferir-pacote .

POR ONDE COMECAR A LER
--------------------------------------------------------------------------------
       README.md               o resumo do projeto
       MANUAL.txt              o manual do operador, de ponta a ponta
       docs/FORMATO.md         o formato em disco, byte a byte
       docs/PLANO.md           o roteiro
       docs/EMPACOTAMENTO.md   como estes zips sao montados e conferidos
       CHANGELOG.md            o que mudou, do mais novo para o mais antigo
================================================================================
TXT

  fecha "$nome"
}

# Confere o que esta em pacotes/: o hash de cada zip e, dentro dele, o
# manifesto. Desempacotar e conferir e a unica prova de que o zip presta --
# olhar o tamanho do arquivo nao e prova de nada.
conferir() {
  local falhas=0 sem_manifesto=0 z nome tmp
  shopt -s nullglob
  local zips=("$SAIDA"/*.zip)
  shopt -u nullglob
  [ ${#zips[@]} -gt 0 ] || { echo "nao ha zip em $SAIDA/"; exit 2; }

  # O conferidor e o proprio phxsql -- o mesmo que viaja dentro do pacote.
  cargo build --release --offline -p phxsql-cli --bin phxsql

  ( cd "$SAIDA" && sha256sum -c SHA256SUMS ) || falhas=$((falhas + 1))
  echo

  tmp=$(mktemp -d)
  for z in "${zips[@]}"; do
    nome=$(basename "$z" .zip)
    echo "== $nome"
    rm -rf "${tmp:?}/$nome"
    # Zip com byte trocado nem sempre abre -- o CRC do proprio ZIP pega parte
    # das adulteracoes antes do manifesto. Isso e resultado, e nao motivo para
    # o conferidor desistir dos outros dois pacotes no meio do caminho.
    if ! unzip -q "$z" -d "$tmp"; then
      echo "   o zip nao abre inteiro -- ver a linha do SHA256SUMS acima"
      falhas=$((falhas + 1))
      continue
    fi
    # Pacote ANTERIOR ao manifesto nao e pacote corrompido: e pacote velho.
    # Contar os dois como "nao confere" ensina quem le a ignorar o conferidor,
    # e conferidor ignorado nao confere nada. A linha de detalhe ja dizia a
    # verdade; era o resumo que confundia.
    if [ ! -f "$tmp/$nome/MANIFESTO.sha256" ]; then
      echo "   SEM MANIFESTO -- pacote anterior a versao que passou a gravar um."
      echo "   Nada a conferir por aqui; a linha do SHA256SUMS acima ainda vale."
      sem_manifesto=$((sem_manifesto + 1))
      continue
    fi
    ./target/release/phxsql conferir-pacote "$tmp/$nome" || falhas=$((falhas + 1))
  done
  rm -rf "$tmp"

  echo
  [ $sem_manifesto -eq 0 ] || \
    echo "$sem_manifesto pacote(s) sem manifesto (anteriores a ele) -- nao conferidos."
  [ $falhas -eq 0 ] || { echo "$falhas pacote(s) NAO conferem."; exit 1; }
  echo "$(( ${#zips[@]} - sem_manifesto )) pacote(s) conferem."
}

# ---------------------------------------------------------------------------
# O dossie, sozinho
#
# Vai separado porque tem publico proprio: quem quer entender o projeto sem
# compilar nada. Leva os GERADORES junto, e nao so o HTML -- pagina sem o que a
# gerou e pagina que ninguem consegue atualizar, e este projeto ja publicou
# numero errado por exatamente isso.
dossie() {
  local nome="phxsql-$VERSAO-dossie"
  local dir="$SAIDA/$nome"
  echo "== dossie"
  rm -rf "$dir"; mkdir -p "$dir/geradores"
  cp docs/dossie/*.html "$dir/"
  cp docs/dossie/*.py docs/dossie/LEIA-ME.md "$dir/geradores/"
  cp CHANGELOG.md "$dir/"
  ( cd "$dir" && find . -type f ! -name "$MANIFESTO" -print0 \
      | sort -z | xargs -0 sha256sum > "$MANIFESTO" )
  echo "   $(grep -c '' "$dir/$MANIFESTO") arquivos no $MANIFESTO"
  ( cd "$SAIDA" && rm -f "$nome.zip" && zip -qr "$nome.zip" "$nome" && rm -rf "$nome" )
  echo "   $SAIDA/$nome.zip"
}

# ---------------------------------------------------------------------------
# O kit: tudo junto, para quem so quer baixar uma coisa
#
# Leva os zips ja prontos em vez de remontar o conteudo -- assim o hash de cada
# um continua sendo o MESMO que o SHA256SUMS publica, e quem baixou o kit pode
# conferir peca por peca contra a lista de fora.
kit() {
  local nome="phxsql-$VERSAO-kit"
  local dir="$SAIDA/$nome"
  echo "== kit"
  rm -rf "$dir"; mkdir -p "$dir/pacotes"
  local p
  for p in "$SAIDA"/phxsql-"$VERSAO"-{fontes,linux,windows,arm64,arm32,dossie}.zip; do
    [ -f "$p" ] && cp "$p" "$dir/pacotes/"
  done
  # A base de conhecimento vai no kit e nao nos fontes: ela nao e o programa, e
  # o que se aprendeu fazendo o programa. Publico diferente.
  [ -d ../base-de-conhecimento ] && cp -r ../base-de-conhecimento "$dir/"
  cp MANUAL.txt README.md CHANGELOG.md "$dir/"
  cat > "$dir/COMECE-AQUI.txt" <<FIM
PhxSql $VERSAO -- kit completo

pacotes/
  phxsql-$VERSAO-fontes.zip    o codigo inteiro, backend e frontend
  phxsql-$VERSAO-linux.zip     binarios x86-64 para Linux
  phxsql-$VERSAO-windows.zip   binarios x86-64 para Windows
  phxsql-$VERSAO-arm64.zip     binarios ARM64 (Raspberry Pi 3/4/5, gateway)
  phxsql-$VERSAO-arm32.zip     binarios ARMv7 (Pi 2, Zero W, roteador)
  phxsql-$VERSAO-dossie.zip    o dossie e os geradores dele

base-de-conhecimento/
  o que se aprendeu FAZENDO isto: os pedidos em ordem, os briefings de
  agente, os scripts de medicao e de prova, e as licoes. Comece pelo
  04-LICOES.md -- e o unico escrito a mao, e o que viaja para outro projeto.

Cada zip traz um MANIFESTO.sha256 por dentro. O SHA256SUMS da pasta pacotes
do site confere o download ANTES de abrir; o manifesto de dentro confere
depois. Os zips daqui sao os MESMOS arquivos, com os mesmos hashes.
FIM
  ( cd "$dir" && find . -type f ! -name "$MANIFESTO" -print0 \
      | sort -z | xargs -0 sha256sum > "$MANIFESTO" )
  echo "   $(grep -c '' "$dir/$MANIFESTO") arquivos no $MANIFESTO"
  ( cd "$SAIDA" && rm -f "$nome.zip" && zip -qr "$nome.zip" "$nome" && rm -rf "$nome" )
  echo "   $SAIDA/$nome.zip"
}

case "$QUAL" in
  conferir) conferir; exit 0 ;;
  linux)   confere_versoes; monta "$ALVO_LINUX" linux "" ;;
  windows) confere_versoes; monta "$ALVO_WINDOWS" windows .exe ;;
  arm64)   confere_versoes; monta "$ALVO_ARM64" arm64 "" sem_odbc ;;
  arm32)   confere_versoes; monta "$ALVO_ARM32" arm32 "" sem_odbc ;;
  fontes)  confere_versoes; fontes ;;
  dossie)  confere_versoes; dossie ;;
  kit)     confere_versoes; kit ;;
  tudo)
    confere_versoes
    monta "$ALVO_LINUX" linux ""
    monta "$ALVO_WINDOWS" windows .exe
    monta "$ALVO_ARM64" arm64 "" sem_odbc
    monta "$ALVO_ARM32" arm32 "" sem_odbc
    fontes
    dossie
    kit
    ;;
  *) echo "uso: $0 [linux|windows|arm64|arm32|fontes|dossie|kit|tudo|conferir]" >&2; exit 2 ;;
esac

# A lista de fora: o hash dos proprios zips, para quem baixou saber que o
# download chegou inteiro ANTES de abrir. O manifesto de dentro nao responde
# essa pergunta -- ele so existe depois do unzip.
( cd "$SAIDA" && rm -f SHA256SUMS && sha256sum ./*.zip | sed 's|\./||' > SHA256SUMS )

echo
ls -lh "$SAIDA"/*.zip
echo
cat "$SAIDA/SHA256SUMS"
