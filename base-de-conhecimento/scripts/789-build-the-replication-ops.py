# Build the replication ops
# 28/08 20:13

import pathlib
p = pathlib.Path("crates/phxsql-server/src/servidor.rs")
s = p.read_text()
s = s.replace("""use crate::valores::{
    bytes_para_hex, json_para_chave, json_para_linha, largura_do_tipo, linha_para_json,
};""", """use crate::valores::{
    bytes_para_hex, hex_para_bytes, json_para_chave, json_para_linha, largura_do_tipo,
    linha_para_json,
};""")
p.write_text(s)
