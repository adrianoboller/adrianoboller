# Render and validate the figures
# 28/08 20:40

import pathlib
p = pathlib.Path("docs/dossie/dossie-phxsql-0.15.html")
s = p.read_text()
s = s.replace('''          <rect x="396" y="214" width="180" height="52" rx="4" fill="none" stroke="var(--log)" stroke-width="1.5" stroke-dasharray="5 3" opacity="0"/>
''', '')
p.write_text(s)
