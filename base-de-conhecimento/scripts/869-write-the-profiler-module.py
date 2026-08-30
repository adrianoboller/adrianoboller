# Write the profiler module
# 28/08 22:56

import pathlib
p = pathlib.Path("crates/phxsql-server/src/lib.rs")
s = p.read_text()
s = s.replace("pub mod pivot;", "pub mod pivot;\npub mod profiler;")
p.write_text(s)
