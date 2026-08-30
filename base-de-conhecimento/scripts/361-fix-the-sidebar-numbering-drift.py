# Fix the sidebar numbering drift
# 28/08 13:16

import pathlib, re
p = pathlib.Path('docs/dossie/dossie-phxsql.html')
s = p.read_text()

# O numero exibido passa a sair do proprio alvo: `#s7` mostra 07. Assim ele
# nao tem como divergir de novo -- e foi o que aconteceu, do item 4 ao 10.
def arruma(m):
    alvo = int(m.group(1))
    return f'<li><a href="#s{alvo}"><span class="n">{alvo:02d}</span>{m.group(3)}</a></li>'

novo, n = re.subn(r'<li><a href="#s(\d+)"><span class="n">([^<]+)</span>([^<]*)</a></li>',
                  arruma, s)
assert n == 19, n
p.write_text(novo)
print(f'{n} itens do indice renumerados a partir do alvo')
