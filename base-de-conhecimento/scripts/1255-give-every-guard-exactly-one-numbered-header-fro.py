# Give every guard exactly one numbered header from its own title
# 30/08 06:22

import re
p='bancada/guardas/catalogo.py'
s=open(p,encoding='utf-8').read()
TRACO='    # ' + '-'*71
# Passada unica e burra: cada guarda ganha exatamente um cabecalho numerado,
# tirado do seu proprio campo "titulo". Nada de adivinhar o que havia antes.
partes = s.split(TRACO + '\n')
saida = [partes[0]]
n = 0
for corpo in partes[1:]:
    tit = re.search(r'"titulo":\s*"([^"]+)"', corpo)
    n += 1
    cab = f'    # {n}. {tit.group(1)}\n' if tit else ''
    # Tira cabecalho antigo que tenha sobrado logo acima deste traco.
    saida[-1] = re.sub(r'\n    # (?:\d+\. )?[^\n-][^\n]*\n$', '\n', saida[-1])
    saida.append(cab + TRACO + '\n' + corpo)
open(p,'w',encoding='utf-8').write(''.join(saida))
print(f"{n} guardas, cada uma com um cabecalho tirado do proprio titulo")
