# Restart with web console enabled
# 28/08 10:39

import json, pathlib
p = pathlib.Path('/tmp/claude-0/-home-user-adrianoboller/34595649-0af6-575a-8f79-80dbe8cb7a5d/scratchpad/gt/config.json')
c = json.loads(p.read_text())
c["web"] = {"ligado": True, "bind": "127.0.0.1:5642", "sessao_minutos": 120}
p.write_text(json.dumps(c, indent=2))
