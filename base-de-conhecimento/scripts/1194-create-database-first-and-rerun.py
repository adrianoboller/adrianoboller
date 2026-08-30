# Create database first and rerun
# 29/08 18:56

import pathlib
p = pathlib.Path("/tmp/claude-0/-home-user-adrianoboller/34595649-0af6-575a-8f79-80dbe8cb7a5d/scratchpad/integra/lgpd.mjs")
t = p.read_text()
t = t.replace("""const r = (await sessao([`{${T},"op":"login","usuario":"adriano","senha":"demo123"}`, `{${T},"op":"criar_tabela""",
"""const r = (await sessao([`{${T},"op":"login","usuario":"adriano","senha":"demo123"}`, `{${T},"op":"criar_database","database":"loja"}`, `{${T},"op":"criar_tabela""")
t = t.replace("}]}`]))[1];", "}]}`]))[2];")
p.write_text(t); print("criar_database incluido")
