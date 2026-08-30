# Fix duplicate and rerun screen tests
# 28/08 11:35

import pathlib
p = pathlib.Path('/tmp/claude-0/-home-user-adrianoboller/34595649-0af6-575a-8f79-80dbe8cb7a5d/scratchpad/telas.mjs')
s = p.read_text()
s = s.replace('''await p.locator('.menubar .titulo:has-text("Ferramentas")').click(); await p.waitForTimeout(200);
await p.locator(`.menubar .menu[data-m="6"] .item:has-text("Editor de menu")`).click(); await p.waitForTimeout(800);''',
'''await p.locator('.menubar .titulo:has-text("Configurações")').click(); await p.waitForTimeout(200);
await p.locator(`.menubar .menu[data-m="6"] .item:has-text("Editor de menu")`).click(); await p.waitForTimeout(800);''')
s = s.replace("""const cfg = await p.$$eval('.menubar .menu:has(.titulo:text-is("Configurações")) .item .rot', ns => ns.map(n=>n.textContent));""",
              """const cfg = await p.$$eval('.menubar .menu[data-m="6"] .item .rot', ns => ns.map(n=>n.textContent));""")
s = s.replace("""await p.locator('.ferramentas .fer:has-text("Gerir Banco")').click(); await p.waitForTimeout(1000);""",
              """await p.locator('.arvore .no.db:has-text("loja")').click(); await p.waitForTimeout(800);
await p.locator('.ferramentas .fer:has-text("Gerir Banco")').click(); await p.waitForTimeout(1000);""")
p.write_text(s)
