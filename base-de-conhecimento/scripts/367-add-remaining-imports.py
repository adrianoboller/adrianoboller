# Add remaining imports
# 28/08 13:28

import pathlib
p = pathlib.Path('crates/phxsql-server/src/servidor.rs')
s = p.read_text()
v = 'use crate::pivot::{Agregador, Campo, Granularidade, Juncao};'
n = ('use std::collections::HashMap;\n\n'
     'use phxsql_core::schema::Schema;\n'
     'use phxsql_core::value::Value;\n\n'
     'use crate::pivot::{Agregador, Campo, Granularidade, Juncao};')
assert s.count(v) == 1
p.write_text(s.replace(v, n))
