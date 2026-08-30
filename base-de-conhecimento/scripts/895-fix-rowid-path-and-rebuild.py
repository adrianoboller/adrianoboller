# Fix RowId path and rebuild
# 28/08 23:53

import pathlib
p = pathlib.Path("crates/phxsql-server/src/servidor.rs")
s = p.read_text()
s = s.replace("fn conferir_versao_pedida(t: &mut Table, p: &Json, rowid: RowId)",
              "fn conferir_versao_pedida(t: &mut Table, p: &Json, rowid: phxsql_core::RowId)", 1)
p.write_text(s)
