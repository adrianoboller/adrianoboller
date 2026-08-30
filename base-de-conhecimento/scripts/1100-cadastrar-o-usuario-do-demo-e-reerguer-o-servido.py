# Cadastrar o usuario do demo e reerguer o servidor
# 29/08 10:21

import json,io
p='$S/demo/config.json'
c=json.load(io.open(p))
c['usuarios']=[{"login":"adriano","nome":"Adriano","id":1,"senha_hash":"$H",
  "bases":[{"database":"*","permissoes":["ler","inserir","alterar","excluir","administrar","diario"]}]}]
json.dump(c, io.open(p,'w'), ensure_ascii=False, indent=1)
print('usuario adriano cadastrado')
