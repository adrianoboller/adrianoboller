# Distinguish 'no manifest' from 'does not verify'
# 30/08 02:54

import io
p="phxsql/empacotar.sh"
s=io.open(p,encoding="utf-8").read()
velho = """    ./target/release/phxsql conferir-pacote "$tmp/$nome" || falhas=$((falhas + 1))
  done
  rm -rf "$tmp"

  echo
  [ $falhas -eq 0 ] || { echo "$falhas pacote(s) NAO conferem."; exit 1; }
  echo "os ${#zips[@]} pacotes conferem."
"""
novo = """    # Pacote ANTERIOR ao manifesto nao e pacote corrompido: e pacote velho.
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
  [ $sem_manifesto -eq 0 ] || \\
    echo "$sem_manifesto pacote(s) sem manifesto (anteriores a ele) -- nao conferidos."
  [ $falhas -eq 0 ] || { echo "$falhas pacote(s) NAO conferem."; exit 1; }
  echo "$(( ${#zips[@]} - sem_manifesto )) pacote(s) conferem."
"""
assert s.count(velho)==1
s=s.replace(velho,novo)
s=s.replace("  local falhas=0 z nome tmp\n","  local falhas=0 sem_manifesto=0 z nome tmp\n",1)
io.open(p,"w",encoding="utf-8").write(s)
print("ok")
