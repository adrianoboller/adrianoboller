# Test the user listing command
# 27/08 19:06

p="/tmp/claude-0/-home-user-adrianoboller/34595649-0af6-575a-8f79-80dbe8cb7a5d/scratchpad/srv2/config.json"
s=open(p).read().replace("TROQUE-ESTE-TOKEN-ANTES-DE-SUBIR","tok").replace('"bind": "0.0.0.0:5000"','"bind": "127.0.0.1:5001"')
open(p,"w").write(s)
