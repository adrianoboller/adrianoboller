# Renumber figures by reading order
# 28/08 13:38

import pathlib, re
p = pathlib.Path('docs/dossie/dossie-phxsql.html')
s = p.read_text()
# Renumera todas as figuras pela ORDEM DE LEITURA, que e a unica que importa
# para quem le. Duas passadas para nao colidir com os numeros existentes.
n = [0]
def marca(m):
    n[0] += 1
    return f'<b>Figura {n[0]}.</b>'
s = re.sub(r'<b>Figura \d+\.</b>', marca, s)
s = s.replace('', '')
p.write_text(s)
print('figuras em ordem de leitura:', n[0])
