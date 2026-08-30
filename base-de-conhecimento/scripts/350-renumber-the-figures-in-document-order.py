# Renumber the figures in document order
# 28/08 13:10

import pathlib, re
p = pathlib.Path('docs/dossie/dossie-phxsql.html')
s = p.read_text()

# A figura nova entra como 5; as de 5 a 16 andam uma casa. Renumera de tras
# para a frente para nao colidir.
for n in range(16, 4, -1):
    v = f'<b>Figura {n}.</b>'
    assert s.count(v) == 1, f'Figura {n}'
    s = s.replace(v, f'<b>Figura {n+1}.</b>')
v = '<b>Figura 17.</b> A tabela de fronteiras'
assert s.count(v) == 1
s = s.replace(v, '<b>Figura 5.</b> A tabela de fronteiras')
p.write_text(s)
print('renumeradas')
