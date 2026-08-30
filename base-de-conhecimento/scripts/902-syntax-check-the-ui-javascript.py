# Syntax-check the UI JavaScript
# 28/08 23:56

import re, pathlib
s = pathlib.Path("crates/phxsql-server/ui/index.html").read_text()
blocos = re.findall(r"<script[^>]*>(.*?)</script>", s, re.S)
print("blocos:", len(blocos), [len(b) for b in blocos])
pathlib.Path("/tmp/claude-0/-home-user-adrianoboller/34595649-0af6-575a-8f79-80dbe8cb7a5d/scratchpad/ui.js").write_text("\n".join(blocos))
