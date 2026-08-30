# Add the sequences screen
# 28/08 14:09

import pathlib
p = pathlib.Path('crates/phxsql-server/ui/index.html'); s = p.read_text()
v = '.linha-tab,.linha-dado{cursor:pointer}'
assert s.count(v) == 1
p.write_text(s.replace(v, '.linha-tab,.linha-dado,.linha-seq{cursor:pointer}\n.linha-seq:hover{background:var(--realce)}'))
