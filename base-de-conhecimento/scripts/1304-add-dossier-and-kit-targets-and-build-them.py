# Add dossier and kit targets and build them
# 30/08 16:36

p='empacotar.sh'
s=open(p,encoding='utf-8').read()

# Dois alvos novos, no mesmo molde dos outros: nada montado a mao.
novo_fn = '''
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
  ( cd "$dir" && find . -type f ! -name "$MANIFESTO" -print0 \\
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
  ( cd "$dir" && find . -type f ! -name "$MANIFESTO" -print0 \\
      | sort -z | xargs -0 sha256sum > "$MANIFESTO" )
  echo "   $(grep -c '' "$dir/$MANIFESTO") arquivos no $MANIFESTO"
  ( cd "$SAIDA" && rm -f "$nome.zip" && zip -qr "$nome.zip" "$nome" && rm -rf "$nome" )
  echo "   $SAIDA/$nome.zip"
}

case "$QUAL" in'''
assert s.count('\ncase "$QUAL" in')==1
s=s.replace('\ncase "$QUAL" in', novo_fn, 1)

velho='''  fontes)  confere_versoes; fontes ;;
  tudo)'''
novo='''  fontes)  confere_versoes; fontes ;;
  dossie)  confere_versoes; dossie ;;
  kit)     confere_versoes; kit ;;
  tudo)'''
assert s.count(velho)==1
s=s.replace(velho,novo)

velho2='''    monta "$ALVO_ARM32" arm32 "" sem_odbc
    fontes
    ;;'''
novo2='''    monta "$ALVO_ARM32" arm32 "" sem_odbc
    fontes
    dossie
    kit
    ;;'''
assert s.count(velho2)==1
s=s.replace(velho2,novo2)
s=s.replace('uso: $0 [linux|windows|arm64|arm32|fontes|tudo|conferir]',
            'uso: $0 [linux|windows|arm64|arm32|fontes|dossie|kit|tudo|conferir]')
open(p,'w',encoding='utf-8').write(s)
print("alvos dossie e kit acrescentados")
