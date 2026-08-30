# Acrescentar o phxsqlcmd ao pacote
# 29/08 11:07

import io
p='empacotar.sh'
s=io.open(p,encoding='utf-8').read()
velho='''  cp "target/$alvo/release/phxsqld$sufixo" "$dir/"
  cp "target/$alvo/release/phxsql$sufixo"  "$dir/"'''
novo='''  cp "target/$alvo/release/phxsqld$sufixo" "$dir/"
  cp "target/$alvo/release/phxsql$sufixo"  "$dir/"
  # O console entrou na 0.18.0 e quase ficou de fora do pacote: o empacotador
  # nao sabia dele. Binario novo se acrescenta AQUI, ou o download mente.
  cp "target/$alvo/release/phxsqlcmd$sufixo" "$dir/"'''
assert s.count(velho)==1
io.open(p,'w',encoding='utf-8').write(s.replace(velho,novo))
print('ok')
