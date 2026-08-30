# Corrigir as permissoes e sondar de novo
# 29/08 10:27

import json,io
p='$S/demo/config.json'
c=json.load(io.open(p))
c['usuarios'][0]['bases']={"*":{"ler":True,"inserir":True,"alterar":True,"excluir":True,
  "criar":True,"administrar":True,"diario":True,"verificar":True,"replicar":True}}
json.dump(c, io.open(p,'w'), ensure_ascii=False, indent=1)
print('bases no formato certo')
