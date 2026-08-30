# Show last two benchmark entries
# 28/08 17:16

import json,io
for r in json.load(io.open('/home/user/adrianoboller/phxsql/bancada/resultados.json',encoding='utf-8'))[-2:]:
    print(r)
