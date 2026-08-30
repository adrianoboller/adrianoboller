# Fix imports
# 28/08 20:13

import pathlib
p = pathlib.Path("crates/phxsql-server/src/servidor.rs")
s = p.read_text()
s = s.replace("use phxsql_store::table::{Table, Visao};",
              "use phxsql_store::log::Operacao;\nuse phxsql_store::table::{Table, Visao};")
s = s.replace("use crate::valores::{json_para_chave, json_para_linha, largura_do_tipo, linha_para_json};",
              "use crate::valores::{\n    bytes_para_hex, json_para_chave, json_para_linha, largura_do_tipo, linha_para_json,\n};")
p.write_text(s)
