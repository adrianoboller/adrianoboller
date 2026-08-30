# Remove orphan conflict markers
# 29/08 17:20

import pathlib
p = pathlib.Path("crates/phxsql-core/src/error.rs")
linhas = [l for l in p.read_text().splitlines(keepends=True)
          if not (l.startswith("<<<<<<< ") or l.startswith(">>>>>>> ") or l.rstrip() == "=======")]
p.write_text("".join(linhas))
print("marcadores orfaos removidos")
