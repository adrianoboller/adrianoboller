# Test cascading replication
# 28/08 20:25

import json, pathlib
p = pathlib.Path("/tmp/claude-0/-home-user-adrianoboller/34595649-0af6-575a-8f79-80dbe8cb7a5d/scratchpad/rep/slave01/config.json")
c = json.loads(p.read_text())
c["replicacao"]["imagem_da_linha"] = True
p.write_text(json.dumps(c, indent=2))
print("slave01 passa a gravar a imagem no proprio diario")
