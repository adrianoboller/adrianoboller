# Point slave03 at slave01
# 28/08 20:25

import json, pathlib
p = pathlib.Path("/tmp/claude-0/-home-user-adrianoboller/34595649-0af6-575a-8f79-80dbe8cb7a5d/scratchpad/rep/slave03/config.json")
c = json.loads(p.read_text())
c["replicacao"]["origens"][0]["porta"] = 5801
c["replicacao"]["origens"][0]["nome"] = "slave01"
p.write_text(json.dumps(c, indent=2))
print("slave03 -> slave01")
