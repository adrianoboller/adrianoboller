# Move the loader to core
# 28/08 19:28

import io
p='crates/phxsql-core/src/carga.rs'
s=io.open(p,encoding='utf-8').read()
s=s.replace("use phxsql_core::error::{PhxError, Result};\nuse phxsql_core::json::Json;",
            "use crate::error::{PhxError, Result};\nuse crate::json::Json;",1)
io.open(p,'w',encoding='utf-8').write(s)
