# Find the table-rights section in HFSQL.md
# 29/08 00:33

import pathlib
p = pathlib.Path("docs/HFSQL.md")
s = p.read_text()
# Encontrar a secao do direito por tabela
import re
m = [l for l in s.split("\n") if l.startswith("### 3.")]
print("\n".join(m))
