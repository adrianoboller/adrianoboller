# Point the server at core's conversions
# 28/08 19:30

import io
p='crates/phxsql-server/src/valores.rs'
s=io.open(p,encoding='utf-8').read()
# o servidor passa a usar as do nucleo
if 'pub use phxsql_core::carga::' not in s:
    s = s.replace('use phxsql_core::json::Json;',
                  'use phxsql_core::json::Json;\n\n// As conversoes de TEXTO moram no nucleo, porque a linha de comando tambem\n// carrega arquivo -- e duas implementacoes divergiriam. Aqui so o reexporte.\npub use phxsql_core::carga::{hex_para_bytes, texto_para_decimal};\nuse phxsql_core::carga::data_de_texto;', 1)
io.open(p,'w',encoding='utf-8').write(s)
