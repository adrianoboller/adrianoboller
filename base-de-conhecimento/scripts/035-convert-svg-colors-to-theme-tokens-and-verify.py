# Convert SVG colors to theme tokens and verify
# 27/08 18:56

import re
p="/tmp/claude-0/-home-user-adrianoboller/34595649-0af6-575a-8f79-80dbe8cb7a5d/scratchpad/artefato/dossie-phxsql.html"
s=open(p).read()
mapa={
 "#2a5f8f":"var(--reg)",
 "#6b4f9c":"var(--ndx)",
 "#9c5f21":"var(--bin)",
 "#3f7a33":"var(--memo)",
 "#9c3a48":"var(--log)",
 "#0b6b74":"var(--acento)",
}
antes=0; depois=0
# So dentro de atributos fill= e stroke=, que so existem no SVG.
def troca(m):
    global depois
    attr, hexa = m.group(1), m.group(2).lower()
    if hexa in mapa:
        depois+=1
        return f'{attr}="{mapa[hexa]}"'
    return m.group(0)
antes=len(re.findall(r'(?:fill|stroke)="#[0-9a-fA-F]{6}"', s))
s=re.sub(r'(fill|stroke)="(#[0-9a-fA-F]{6})"', troca, s)
open(p,'w').write(s)
print(f"atributos de cor no SVG: {antes} encontrados, {depois} passados para tokens")
