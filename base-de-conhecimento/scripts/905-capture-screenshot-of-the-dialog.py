# Capture screenshot of the dialog
# 29/08 00:01

import pathlib
p = pathlib.Path("$SC/prova-conflito.mjs")
s = p.read_text()
s = s.replace("""console.log('05 caixa de conflito apareceu');""",
"""console.log('05 caixa de conflito apareceu');
await esperar(400);
await p.screenshot({ path: \`\${S}/caixa-conflito.png\` });""")
p.write_text(s)
