# Add imports and build
# 29/08 00:10

import pathlib
p = pathlib.Path("crates/phxsql-store/src/ndx.rs")
s = p.read_text()
s = s.replace("use std::fs::{File, OpenOptions};",
              "use std::collections::{HashMap, VecDeque};\nuse std::fs::{File, OpenOptions};", 1)
p.write_text(s)
