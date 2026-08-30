# Find a config template to start a server
# 30/08 15:33

import json
c=json.load(open('exemplos/Config_exemplo_isolado.json')) if __import__('os').path.exists('exemplos/Config_exemplo_isolado.json') else None
print("modelo isolado:", "achado" if c else "nao achado")
if c: print(json.dumps(c, indent=1)[:600])
