# Run the ARM64 server under emulation and talk to it
# 30/08 15:38

import json
c=json.load(open("config.json"))
c["bind"]="127.0.0.1:6992"
c["base"]="$S/dados"
if isinstance(c.get("web"),dict): c["web"]["ligado"]=False; c["web"]["bind"]="127.0.0.1:6993"
json.dump(c,open("config.json","w"),indent=1)
