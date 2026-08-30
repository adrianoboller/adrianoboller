# Keep the partition checkbox on one line
# 28/08 11:45

import pathlib
p = pathlib.Path('crates/phxsql-server/ui/index.html')
s = p.read_text()
v = '.secao .chk{margin-left:auto;font-weight:400}'
n = '.secao .chk{margin-left:auto;font-weight:400;white-space:nowrap;flex:0 0 auto}'
assert s.count(v) == 1
p.write_text(s.replace(v, n))
print('ok')
