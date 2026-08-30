# Fix selector and rerender
# 28/08 13:15

import pathlib
p = pathlib.Path('/tmp/claude-0/-home-user-adrianoboller/34595649-0af6-575a-8f79-80dbe8cb7a5d/scratchpad/dossie.mjs')
s = p.read_text()
s = s.replace("""  const fig5 = await p.locator('figure:has(figcaption:text-matches("Figura 5"))');
  await fig5.scrollIntoViewIfNeeded(); await p.waitForTimeout(300);
  await fig5.screenshot({ path: `d-figura5-${tema}.png` });""",
"""  const fig5 = p.locator('figure').nth(4);
  await fig5.scrollIntoViewIfNeeded(); await p.waitForTimeout(300);
  await fig5.screenshot({ path: `d-figura5-${tema}.png` });
  // e a secao nova da gestao do banco
  await p.locator('h3:has-text("Gerir o banco")').scrollIntoViewIfNeeded();
  await p.waitForTimeout(300);
  await p.screenshot({ path: `d-gestao-${tema}.png`, clip:{x:0,y:0,width:1280,height:900} });""")
p.write_text(s)
