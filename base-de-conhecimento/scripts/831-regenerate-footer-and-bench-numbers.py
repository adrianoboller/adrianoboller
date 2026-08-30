# Regenerate footer and bench numbers
# 28/08 20:47

import pathlib
p = pathlib.Path("docs/dossie/numeros-do-projeto.py")
s = p.read_text()
s = s.replace("permissões em <code>docs/USUARIOS.md</code>, desenho da replicação em",
              "permissões em <code>docs/USUARIOS.md</code>, a replicação em")
p.write_text(s)
