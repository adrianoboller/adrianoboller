# Final dossier render check
# 28/08 13:17

import pathlib
p = pathlib.Path('/tmp/claude-0/-home-user-adrianoboller/34595649-0af6-575a-8f79-80dbe8cb7a5d/scratchpad/dossie.mjs')
s = p.read_text()
s = s.replace("""  await p.locator('h3:has-text("Gerir o banco")').scrollIntoViewIfNeeded();
  await p.waitForTimeout(300);
  await p.screenshot({ path: `d-gestao-${tema}.png`, clip:{x:0,y:0,width:1280,height:900} });""",
"""  await p.locator('h3:has-text("Gerir o banco")').scrollIntoViewIfNeeded();
  await p.waitForTimeout(300);
  await p.screenshot({ path: `d-gestao-${tema}.png`, clip:{x:0,y:0,width:1280,height:900} });
  // o indice lateral, que estava com os numeros fora de ordem
  await p.evaluate(() => window.scrollTo(0,0)); await p.waitForTimeout(300);
  const nums = await p.$$eval('.indice .n', ns => ns.map(n => n.textContent));
  if (tema === 'claro') console.log('índice:', nums.join(' '));
  await p.locator('.indice').screenshot({ path: `d-indice-${tema}.png` });
  // e a capa
  await p.screenshot({ path: `d-capa-${tema}.png`, clip:{x:0,y:0,width:1280,height:820} });""")
p.write_text(s)
