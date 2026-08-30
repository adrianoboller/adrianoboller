# Add the create-table and insert scenes
# 28/08 21:30

import pathlib
p = pathlib.Path("/tmp/claude-0/-home-user-adrianoboller/34595649-0af6-575a-8f79-80dbe8cb7a5d/scratchpad/video/roteiro.mjs")
s = p.read_text()
s = s.replace("return await p.evaluate(([o, a]) => window.api(o, a), [op, args]);",
              "return await p.evaluate(([o, a]) => api(o, a), [op, args]);")
s = s.replace("await p.evaluate(() => atualizarArvore && atualizarArvore());",
              "await p.evaluate(() => montarArvore(false));")
p.write_text(s)
