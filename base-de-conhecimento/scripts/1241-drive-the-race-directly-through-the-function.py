# Drive the race directly through the function
# 30/08 04:19

import io
p="/tmp/claude-0/-home-user-adrianoboller/34595649-0af6-575a-8f79-80dbe8cb7a5d/scratchpad/prova-atropelo.mjs"
s=io.open(p,encoding="utf-8").read()
s=s.replace("""await p.locator('#arvore').getByText('Painel', { exact:true }).click().catch(()=>{});
await p.waitForTimeout(120);
await p.locator('#arvore').getByText('Usuários', { exact:true }).click();""",
"""await p.evaluate(() => abrirAdmin('painel'));
await p.waitForTimeout(120);
await p.evaluate(() => abrirAdmin('usuarios'));""")
io.open(p,"w",encoding="utf-8").write(s)
