# Regroup the diamond labels and re-render
# 29/08 01:43

import pathlib
p = pathlib.Path("$SC/caminho-da-insercao.html")
s = p.read_text()
alvo = '''      <line x1="620" y1="494" x2="654" y2="494" stroke="var(--log)" stroke-width="1.4" marker-end="url(#ci-seta-log)"/>
      <text x="636" y="484" font-size="10.5" fill="var(--log)">sim</text>
      <text x="636" y="514" font-size="11" fill="var(--log)">recusa 3002,</text>
      <text x="636" y="528" font-size="11" fill="var(--log)">sem gravar nada</text>
      <text x="636" y="552" font-size="13" fill="currentColor" font-family="IBM Plex Mono, monospace">0,7 µs</text>
      <text x="636" y="566" font-size="10.5" fill="currentColor" opacity=".55">4,0% do total</text>'''
novo = '''      <text x="636" y="466" font-size="13" fill="currentColor" font-family="IBM Plex Mono, monospace">0,7 µs</text>
      <text x="636" y="480" font-size="10.5" fill="currentColor" opacity=".55">4,0% do total</text>
      <line x1="620" y1="494" x2="654" y2="494" stroke="var(--log)" stroke-width="1.4" marker-end="url(#ci-seta-log)"/>
      <text x="636" y="512" font-size="11" fill="var(--log)">sim: recusa 3002,</text>
      <text x="636" y="526" font-size="11" fill="var(--log)">sem gravar nada</text>'''
assert s.count(alvo) == 1
s = s.replace(alvo, novo, 1)
p.write_text(s)
print("ok")
