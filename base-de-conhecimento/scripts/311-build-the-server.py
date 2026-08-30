# Build the server
# 28/08 11:22

import pathlib
p = pathlib.Path('crates/phxsql-server/src/servidor.rs')
s = p.read_text()
s = s.replace('use crate::valores::largura_do_tipo;\nuse crate::valores::{json_para_chave, json_para_linha, linha_para_json};',
              'use crate::valores::{json_para_chave, json_para_linha, largura_do_tipo, linha_para_json};')
s = s.replace('        let pag = e.paginacao();\n        let _ = &pag;', '        let pag = e.paginacao();')
p.write_text(s)
