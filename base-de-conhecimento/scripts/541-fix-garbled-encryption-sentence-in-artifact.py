# Fix garbled encryption sentence in artifact
# 28/08 17:16

import io
p = 'melhorias.html'
s = io.open(p, encoding='utf-8').read()
velho = 'Aqui já existem AES-menos-nada mas há SHA-256, HMAC e PBKDF2 escritos — falta a cifra de bloco em si.'
novo = 'Aqui ainda não existe cifra nenhuma no arquivo, mas SHA-256, HMAC e PBKDF2 já estão escritos à mão no projeto — falta a cifra de bloco em si.'
assert velho in s
s = s.replace(velho, novo)
io.open(p, 'w', encoding='utf-8').write(s)
print('ok')
