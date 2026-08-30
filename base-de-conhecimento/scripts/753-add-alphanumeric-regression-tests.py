# Add alphanumeric regression tests
# 28/08 19:47

import pathlib
p = pathlib.Path("crates/phxsql-store/tests/alfanumerica.rs")
s = p.read_text()
s = s.replace("use phxsql_store::table::{Table, Visao};",
              "use phxsql_store::table::{Salto, Table, Visao};")
p.write_text(s)
