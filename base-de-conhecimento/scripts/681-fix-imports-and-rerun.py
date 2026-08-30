# Fix imports and rerun
# 28/08 18:53

import io
p='crates/phxsql-store/src/reg.rs'
s=io.open(p,encoding='utf-8').read()
s=s.replace("use std::path::{Path, PathBuf};",
            "use std::fs::File;\nuse std::io::Read;\nuse std::path::{Path, PathBuf};",1)
io.open(p,'w',encoding='utf-8').write(s)
