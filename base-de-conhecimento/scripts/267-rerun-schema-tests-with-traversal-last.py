# Rerun schema tests with traversal last
# 28/08 10:57

import pathlib
p = pathlib.Path('/tmp/claude-0/-home-user-adrianoboller/34595649-0af6-575a-8f79-80dbe8cb7a5d/scratchpad/schema.mjs')
s = p.read_text()
bloco = """try { await api('criar_schema', { database:'loja', schema:'../fora' }); console.log('ERRO: aceitou travessia'); }
catch (e) { console.log('travessia recusada:', e.message); }
"""
assert s.count(bloco) == 1
s = s.replace(bloco, "")
s += "\n// por ultimo: a travessia bloqueia o IP, entao nada roda depois dela\n" + bloco
p.write_text(s)
