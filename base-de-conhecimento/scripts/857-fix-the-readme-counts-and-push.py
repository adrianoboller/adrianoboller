# Fix the README counts and push
# 28/08 22:43

import pathlib
p = pathlib.Path("README.md")
s = p.read_text()
s = s.replace("**369 testes** só nele\n(`phxsql-core` 163 + `phxsql-store` 206), **573 no projeto inteiro**",
              "**370 testes** só nele\n(`phxsql-core` 163 + `phxsql-store` 207), **574 no projeto inteiro**")
p.write_text(s)
print([l for l in s.splitlines() if "testes" in l][:2])
