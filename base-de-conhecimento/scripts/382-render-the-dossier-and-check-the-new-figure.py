# Render the dossier and check the new figure
# 28/08 13:39

import pathlib
p = pathlib.Path('/tmp/claude-0/-home-user-adrianoboller/34595649-0af6-575a-8f79-80dbe8cb7a5d/scratchpad/dossie.mjs')
s = p.read_text()
s = s.replace("""  const fig5 = p.locator('figure').nth(4);""", """  const fig5 = p.locator('figure').nth(17);""")
s = s.replace("""  await p.locator('h3:has-text("Gerir o banco")').scrollIntoViewIfNeeded();""",
              """  await p.locator('h3:has-text("Tabela dinâmica")').scrollIntoViewIfNeeded();""")
s = s.replace("d-figura5-", "d-figpivot-").replace("d-gestao-", "d-pivotsec-")
p.write_text(s)
