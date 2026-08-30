# Finish the delete scenes
# 28/08 21:31

import pathlib
p = pathlib.Path("/tmp/claude-0/-home-user-adrianoboller/34595649-0af6-575a-8f79-80dbe8cb7a5d/scratchpad/video/roteiro.mjs")
s = p.read_text()
s = s.replace("""const motivo = p.locator('#motivoExcl, [id*="motivo"]').first();
if (await motivo.count()) { await motivo.fill('pedido de remoção do titular'); await esperar(1100); }""",
"""await p.locator('#excMotivo').fill('pedido de remoção do titular');
await esperar(1300);""")
p.write_text(s)
