# Fix the temp-dir usage
# 28/08 11:21

import pathlib, re
p = pathlib.Path('crates/phxsql-store/tests/paginacao_log_reindex.rs')
s = p.read_text()
# cada teste novo ganha um rotulo proprio
rotulos = ['periodo-mes', 'periodo-atrasada', 'periodo-teto', 'periodo-reabrir', 'periodo-bimestre']
i = iter(rotulos)
s = re.sub(r'    let dir = DirTemp::nova\(\);', lambda m: f'    let dir = DirTemp::novo("{next(i)}");', s)
s = s.replace('dir.path()', 'dir.0.as_path()')
p.write_text(s)
print('ok')
