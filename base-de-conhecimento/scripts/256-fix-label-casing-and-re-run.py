# Fix label casing and re-run
# 28/08 10:45

import pathlib
p = pathlib.Path('/tmp/claude-0/-home-user-adrianoboller/34595649-0af6-575a-8f79-80dbe8cb7a5d/scratchpad/gestao.mjs')
s = p.read_text()
s = s.replace("""await p.screenshot({ path: 'gestao-nova.png', clip: { x: 250, y: 110, width: 1030, height: 620 } });""",
"""await p.screenshot({ path: 'gestao-nova.png', clip: { x: 250, y: 110, width: 1140, height: 640 } });""")
p.write_text(s)
