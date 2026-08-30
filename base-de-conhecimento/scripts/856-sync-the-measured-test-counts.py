# Sync the measured test counts
# 28/08 22:43

import pathlib
p = pathlib.Path("README.md")
s = p.read_text()
s = s.replace("**363 testes** só nele\n(`phxsql-core` 163 + `phxsql-store` 200), **567 no projeto inteiro**",
              "**370 testes** só nele\n(`phxsql-core` 163 + `phxsql-store` 207), **574 no projeto inteiro**")
p.write_text(s)
