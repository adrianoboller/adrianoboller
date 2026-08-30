# Add the imports
# 28/08 13:28

import pathlib
p = pathlib.Path('crates/phxsql-server/src/servidor.rs')
s = p.read_text()
v = 'use crate::valores::{json_para_chave, json_para_linha, largura_do_tipo, linha_para_json};'
n = ('use crate::pivot::{Agregador, Campo, Granularidade, Juncao};\n'
     'use crate::valores::{json_para_chave, json_para_linha, largura_do_tipo, linha_para_json};')
assert s.count(v) == 1
p.write_text(s.replace(v, n))
