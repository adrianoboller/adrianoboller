# Check sidebar titles against section headings
# 28/08 13:16

import pathlib, re
s = pathlib.Path('docs/dossie/dossie-phxsql.html').read_text()
indice = re.findall(r'<li><a href="#s(\d+)"><span class="n">\d+</span>\s*([^<]+)</a></li>', s)
secoes = re.findall(r'<section id="s(\d+)">.*?<h2>(.*?)</h2>', s, re.S)
sec = {n: re.sub(r'<[^>]+>', '', t).replace('&#171;','«').replace('&#187;','»') for n, t in secoes}
print(f'{"#":>3}  {"índice":<26} {"seção":<50}')
for n, rot in indice:
    t = sec.get(n, '???')
    marca = '' if rot.strip().lower() in t.lower() else '  <-- não bate'
    print(f'{n:>3}  {rot.strip():<26} {t[:48]:<50}{marca}')
