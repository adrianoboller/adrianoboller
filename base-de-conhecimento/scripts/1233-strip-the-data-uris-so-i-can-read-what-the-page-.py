# Strip the data URIs so I can read what the page says
# 30/08 03:21

import io,re
s=io.open("phxsql/docs/dossie/dossie-phxsql-0.18.html",encoding="utf-8").read()
print("bytes totais:", len(s))
semb64 = re.sub(r'data:image/[a-z]+;base64,[A-Za-z0-9+/=]+', 'DATA-URI', s)
print("sem as imagens:", len(semb64))
io.open("/tmp/dossie-texto.html","w",encoding="utf-8").write(semb64)
# so o texto visivel
t = re.sub(r'<script.*?</script>|<style.*?</style>', '', semb64, flags=re.S)
t = re.sub(r'<[^>]+>', ' ', t)
t = re.sub(r'\s+', ' ', t)
print("texto visivel:", len(t), "caracteres")
io.open("/tmp/dossie-texto.txt","w",encoding="utf-8").write(t)
