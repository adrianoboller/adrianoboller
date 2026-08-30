# Enable the web interface and restart
# 27/08 21:53

import json, pathlib
p = pathlib.Path("/tmp/claude-0/-home-user-adrianoboller/34595649-0af6-575a-8f79-80dbe8cb7a5d/scratchpad/espelho/srv/config.json")
c = json.loads(p.read_text())
c["web"] = {"ligado": True, "bind": "127.0.0.1:5501"}
p.write_text(json.dumps(c, indent=2))
print("web ligada")
