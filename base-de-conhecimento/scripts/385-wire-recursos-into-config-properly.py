# Wire Recursos into Config properly
# 28/08 13:48

import pathlib
p = pathlib.Path('crates/phxsql-server/src/config.rs')
linhas = p.read_text().split('\n')
# linha 592 (1-based) e a do Default do Config; a 427 e a do Recursos
assert linhas[591].strip() == 'conexoes_max: 64,', linhas[591]
linhas.insert(592, '            recursos: Recursos::default(),')
p.write_text('\n'.join(linhas))
print('Default do Config')
