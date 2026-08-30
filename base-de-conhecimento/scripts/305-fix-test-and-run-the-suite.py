# Fix test and run the suite
# 28/08 11:19

import pathlib
p = pathlib.Path('crates/phxsql-store/tests/paginacao_log_reindex.rs')
s = p.read_text()
v = 'Some(p) => e.com_paginacao(p),'
assert s.count(v) == 1
p.write_text(s.replace(v, 'Some(p) => e.com_paginacao(p).unwrap(),'))
