# Update the old test for the partition checkbox
# 28/08 11:58

import pathlib
p = pathlib.Path('/tmp/claude-0/-home-user-adrianoboller/34595649-0af6-575a-8f79-80dbe8cb7a5d/scratchpad/gestao.mjs')
s = p.read_text()
v = """await p.fill('#nt_nome', 'pedidos');
await p.fill('#nt_pag', '1000');"""
n = """await p.fill('#nt_nome', 'pedidos');
// A partição agora é opt-in: só aparece com o check marcado.
await p.locator('#nt_particionada').check(); await p.waitForTimeout(400);
await p.fill('#nt_pag', '1000');"""
assert s.count(v) == 1
p.write_text(s.replace(v, n))
