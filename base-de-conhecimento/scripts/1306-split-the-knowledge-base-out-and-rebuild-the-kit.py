# Split the knowledge base out and rebuild the kit
# 30/08 16:38

p='empacotar.sh'
s=open(p,encoding='utf-8').read()
velho='''  # A base de conhecimento vai no kit e nao nos fontes: ela nao e o programa, e
  # o que se aprendeu fazendo o programa. Publico diferente.
  [ -d ../base-de-conhecimento ] && cp -r ../base-de-conhecimento "$dir/"'''
novo='''  [ -f "$SAIDA/phxsql-$VERSAO-conhecimento.zip" ] \\
    && cp "$SAIDA/phxsql-$VERSAO-conhecimento.zip" "$dir/pacotes/"'''
assert s.count(velho)==1, "ancora do kit nao achada"
s=s.replace(velho,novo)
alvo='''# ---------------------------------------------------------------------------
# O kit: tudo junto'''
conh = '''# ---------------------------------------------------------------------------
# A base de conhecimento, sozinha
#
# Pacote proprio porque o publico e outro: ela nao e o programa, e o que se
# aprendeu FAZENDO o programa -- os pedidos em ordem, os briefings de agente,
# os scripts de medicao e de prova, e as licoes. Serve para o proximo projeto,
# nao para rodar este.
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
s=s.replace('  kit)     confere_versoes; kit ;;',
            '  conhecimento) confere_versoes; conhecimento ;;\n  kit)     confere_versoes; kit ;;')
s=s.replace('    fontes\n    dossie\n    kit','    fontes\n    dossie\n    conhecimento\n    kit')
s=s.replace('|fontes|dossie|kit|tudo|','|fontes|dossie|conhecimento|kit|tudo|')
s=s.replace('''base-de-conhecimento/
  o que se aprendeu FAZENDO isto''','''  phxsql-$VERSAO-conhecimento.zip
                               o que se aprendeu FAZENDO isto''')
open(p,'w',encoding='utf-8').write(s)
print("ok")
