# Add the new doc to the footer
# 28/08 21:26

import pathlib
p = pathlib.Path("docs/dossie/numeros-do-projeto.py")
s = p.read_text()
s = s.replace("""  a revisão contra os motores maduros em <code>docs/COMPARACAO.md</code>,""",
"""  a revisão contra os motores maduros em <code>docs/COMPARACAO.md</code>,
  onde a escrita dói em <code>docs/DESEMPENHO.md</code>,""")
p.write_text(s)
