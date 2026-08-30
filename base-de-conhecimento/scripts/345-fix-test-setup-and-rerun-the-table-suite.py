# Fix test setup and rerun the table suite
# 28/08 11:56

import pathlib
p = pathlib.Path('/tmp/claude-0/-home-user-adrianoboller/34595649-0af6-575a-8f79-80dbe8cb7a5d/scratchpad/gestao.mjs')
s = p.read_text()
v = """const bt = p.locator('.ferramentas .fer:has-text("Tabelas")');"""
n = """await p.locator('.arvore .no.db:has-text("loja")').click(); await p.waitForTimeout(700);
const bt = p.locator('.ferramentas .fer:has-text("Tabelas")').first();"""
assert s.count(v) == 1
s = s.replace(v, n)
s = s.replace("""await p.locator('.ferramentas .fer:has-text("Tabelas")').click();""",
              """await p.locator('.ferramentas .fer:has-text("Tabelas")').first().click();""")
p.write_text(s)
