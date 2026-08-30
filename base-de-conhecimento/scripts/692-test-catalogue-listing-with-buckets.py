# Test catalogue listing with buckets
# 28/08 19:03

import io
p='crates/phxsql-store/src/catalogo.rs'
s=io.open(p,encoding='utf-8').read()
s=s.replace("use phxsql_core::error::{PhxError, Result};",
            "use phxsql_core::error::{PhxError, Result};\nuse phxsql_core::paginacao::BALDES;",1)
io.open(p,'w',encoding='utf-8').write(s)
