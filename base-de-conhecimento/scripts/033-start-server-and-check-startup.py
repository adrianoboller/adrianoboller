# Start server and check startup
# 27/08 18:45

import json,re,io
p="/tmp/claude-0/-home-user-adrianoboller/34595649-0af6-575a-8f79-80dbe8cb7a5d/scratchpad/servidor/config.json"
s=open(p).read().replace("TROQUE-ESTE-TOKEN-ANTES-DE-SUBIR","segredo-de-teste").replace('"bind": "0.0.0.0:5000"','"bind": "127.0.0.1:5000"')
open(p,"w").write(s)
