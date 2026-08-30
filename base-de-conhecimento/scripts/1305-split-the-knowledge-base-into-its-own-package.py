# Split the knowledge base into its own package
# 30/08 16:37

p='empacotar.sh'
s=open(p,encoding='utf-8').read()

# A base de conhecimento sai do kit e vira pacote proprio. O motivo ja estava
# escrito no comentario dela -- publico diferente --, e o limite de envio so
# tornou a separacao obrigatoria em vez de opcional.
velho='''  # A base de conhecimento vai no kit e nao nos fontes: ela nao e o programa, e
  # o que se aprendeu fazendo o programa. Publico diferente.
  [ -d ../base-de-conhecimento ] && cp -r ../base-de-conhecimento "$dir/"'''
novo='''  [ -f "$SAIDA/phxsql-$VERSAO-conhecimento.zip" ] \\
    && cp "$SAIDA/phxsql-$VERSAO-conhecimento.zip" "$dir/pacotes/"'''
assert s.count(velho)==1
s=s.replace(velho,novo)

alvo='''# ---------------------------------------------------------------------------
# O kit: tudo junto'''
conh = '''# ---------------------------------------------------------------------------
# A base de conhecimento, sozinha
#
# Pacote proprio porque o publico e outro: ela nao e o programa, e o que se
# aprendeu FAZENDO o programa -- os pedidos em ordem, os briefings de agente, os
# scripts de medicao e de prova, e as licoes. Serve para o proximo projeto, nao
# para rodar este.
conhecimento() {
  local nome="phxsql-$VERSAO-conhecimento"
  local dir="$SAIDA/$nome"
  echo "== base de conhecimento"
  [ -d ../base-de-conhecimento ] || { echo "   nao ha base-de-conhecimento/"; return 0; }
  rm -rf "$dir"; mkdir -p "$dir"
  cp -r ../base-de-conhecimento/. "$dir/"
  ( cd "$dir" && find . -type f ! -name "$MANIFESTO" -print0 \\
      | sort -z | xargs -0 sha256sum > "$MANIFESTO" )
  echo "   $(grep -c '' "$dir/$MANIFESTO") arquivos no $MANIFESTO"
  ( cd "$SAIDA" && rm -f "$nome.zip" && zip -qr "$nome.zip" "$nome" && rm -rf "$nome" )
  echo "   $SAIDA/$nome.zip"
}

''' + alvo
assert s.count(alvo)==1
s=s.replace(alvo, conh, 1)

s=s.replace('''  kit)     confere_versoes; kit ;;''',
            '''  conhecimento) confere_versoes; conhecimento ;;
  kit)     confere_versoes; kit ;;''')
s=s.replace('''    fontes
    dossie
    kit''','''    fontes
    dossie
    conhecimento
    kit''')
s=s.replace('|fontes|dossie|kit|tudo|','|fontes|dossie|conhecimento|kit|tudo|')
# O COMECE-AQUI passa a citar o zip, e nao a pasta.
s=s.replace('''base-de-conhecimento/
  o que se aprendeu FAZENDO isto''','''  phxsql-$VERSAO-conhecimento.zip
                               o que se aprendeu FAZENDO isto''')
open(p,'w',encoding='utf-8').write(s)
print("alvo conhecimento acrescentado; kit passa a leva-lo como zip")
