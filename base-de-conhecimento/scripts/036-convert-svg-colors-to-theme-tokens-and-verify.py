# Convert SVG colors to theme tokens and verify
# 27/08 18:56

import re
p="/tmp/claude-0/-home-user-adrianoboller/34595649-0af6-575a-8f79-80dbe8cb7a5d/scratchpad/artefato/dossie-phxsql.html"
s=open(p).read()
raiz=re.search(r':root\{(.*?)\}', s, re.S).group(1)
tokens_base=set(re.findall(r'(--[a-z0-9-]+):', raiz))
usados=set(re.findall(r'var\((--[a-z0-9-]+)\)', s))
faltando=usados-tokens_base
print("tokens usados sem definicao no :root base:", faltando or "nenhum")
