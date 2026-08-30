# Add a ready-to-run demo to the binary packages
# 29/08 01:21

import pathlib
p = pathlib.Path("empacotar.sh")
s = p.read_text()

alvo = '''  ( cd "$SAIDA" && zip -qr "$nome.zip" "$nome" )
  rm -rf "$dir"
  echo "   $SAIDA/$nome.zip"
}'''
novo = '''  demonstracao "$dir" "$rotulo" "$sufixo"

  ( cd "$SAIDA" && zip -qr "$nome.zip" "$nome" )
  rm -rf "$dir"
  echo "   $SAIDA/$nome.zip"
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
  local hash
  hash=$(echo "demo" | ./target/release/phxsqld --senha | sed 's/.*: "//;s/"//')

  mkdir -p "$dir/demonstracao"
  cat > "$dir/demonstracao/config.json" <<JSON
{
  "_comentario": "AMBIENTE DE DEMONSTRACAO -- nao use em producao. Escuta so em 127.0.0.1; a senha esta no COMECE-AQUI.txt. Para um servidor de verdade, gere o seu com: phxsqld --exemplo 1 > config.json",

  "bind": "127.0.0.1:5000",
  "base": "dados",
  "token": "demo",

  "web": { "ligado": true, "bind": "127.0.0.1:8080", "sessao_min": 60 },

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
    cat > "$dir/COMECE-AQUI.txt" <<'TXT'
================================================================================
PHXSQL -- COMO VER A TELA EM UM MINUTO                                  Windows
================================================================================

1. Abra o Prompt de Comando NESTA PASTA e rode:

       cd demonstracao
       ..\phxsqld.exe

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
ISTO E UMA DEMONSTRACAO, E NAO UM SERVIDOR DE PRODUCAO
--------------------------------------------------------------------------------
Ela escuta so em 127.0.0.1 -- so alcanca quem esta NESTE computador --, e a
senha esta escrita aqui em cima. Para um servidor de verdade:

       phxsqld.exe --exemplo 1 > config.json      gera o modelo comentado
       phxsqld.exe --senha                        gera o hash de uma senha

e leia a secao 7 do MANUAL.txt, que explica bind, token, usuarios e permissoes.
A senha NUNCA fica em texto puro no config.json -- so o hash.

Os dois programas:

       phxsqld.exe    o servidor
       phxsql.exe     a linha de comando (10 comandos; rode sem argumentos)
================================================================================
TXT
  else
    cat > "$dir/COMECE-AQUI.txt" <<'TXT'
================================================================================
PHXSQL -- COMO VER A TELA EM UM MINUTO                                    Linux
================================================================================

1. No terminal, NESTA PASTA:

       chmod +x phxsqld phxsql
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
ISTO E UMA DEMONSTRACAO, E NAO UM SERVIDOR DE PRODUCAO
--------------------------------------------------------------------------------
Ela escuta so em 127.0.0.1 -- so alcanca quem esta NESTA maquina --, e a senha
esta escrita aqui em cima. Para um servidor de verdade:

       ./phxsqld --exemplo 1 > config.json      gera o modelo comentado
       ./phxsqld --senha                        gera o hash de uma senha

e leia a secao 7 do MANUAL.txt, que explica bind, token, usuarios e permissoes.
A senha NUNCA fica em texto puro no config.json -- so o hash.

Os dois programas:

       phxsqld    o servidor
       phxsql     a linha de comando (10 comandos; rode sem argumentos)
================================================================================
TXT
  fi
}'''
assert s.count(alvo) == 1
s = s.replace(alvo, novo, 1)
p.write_text(s)
print("ok")
