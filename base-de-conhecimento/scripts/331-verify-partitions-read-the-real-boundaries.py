# Verify partitions read the real boundaries
# 28/08 11:42

import pathlib
p = pathlib.Path('/tmp/claude-0/-home-user-adrianoboller/34595649-0af6-575a-8f79-80dbe8cb7a5d/scratchpad/final.mjs')
s = p.read_text()
s = s.replace("""const vols = await p.$$eval('#painel tbody tr td:nth-child(2) code', ns => ns.map(n=>n.textContent));
console.log('3. volumes na tela:', vols.join(', '));""",
"""const vols = await p.$$eval('#painel tbody tr', ts => ts.map(t =>
  [...t.querySelectorAll('td')].map(d=>d.textContent.trim()).join(' | ')));
console.log('3. volumes na tela:'); vols.forEach(v => console.log('    ', v));
await tela('f-particoes.png', 'partições por período', 700);""")
p.write_text(s)
