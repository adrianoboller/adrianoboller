# Start integration validation server
# 29/08 18:26

import json
c = {"bind":"127.0.0.1:5399","base":"$S/dados","token":"segredo",
     "web":{"ligado":True,"bind":"127.0.0.1:5799"},
     "usuarios":[{"login":"adriano","senha":"demo123",
       "bases":{"*":{"ler":True,"inserir":True,"alterar":True,"excluir":True,"criar":True,"administrar":True}}}]}
open("$S/config.json","w").write(json.dumps(c, indent=2))
