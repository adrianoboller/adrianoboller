# Record the video in the pending list and push
# 28/08 22:45

import pathlib, re
p = pathlib.Path("docs/PENDENCIAS.md")
s = p.read_text()
antigo = """| ☑️ | 67 | **Botão e menu Tabelas**"""
novo = """| ☑️ | 115 | **Vídeo longo em MP4, do login à replicação** | `docs/video/`: 5m13s gravados contra o servidor de verdade, com o Playwright dirigindo a interface e a legenda injetada na própria página. Dezessete capítulos, e o 16 é o que nenhum vídeo de produto tem — o que ainda falta. **Ele achou três defeitos** que ler o código não acharia |
| ☑️ | 67 | **Botão e menu Tabelas**"""
assert antigo in s
s = s.replace(antigo, novo)
linhas = [l for l in s.splitlines() if re.match(r"^\| (☑️|◐|☐) \| \d+ \|", l)]
from collections import Counter
c = Counter(l.split("|")[1].strip() for l in linhas)
s = re.sub(r"\*\*\d+ feitos · \d+ parciais · \d+ planejados\*\*, de \d+ pedidos\.",
           f"**{c['☑️']} feitos · {c['◐']} parciais · {c['☐']} planejados**, de {len(linhas)} pedidos.", s)
p.write_text(s)
print(f"{c['☑️']} feitos · {c['◐']} parciais · {c['☐']} planejados, de {len(linhas)}")
