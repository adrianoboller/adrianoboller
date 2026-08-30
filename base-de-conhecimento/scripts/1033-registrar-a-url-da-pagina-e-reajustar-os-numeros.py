# Registrar a URL da pagina e reajustar os numeros
# 29/08 03:20

import io
URL='https://claude.ai/code/artifact/d6c8f13c-e4a2-444e-9f19-0e047e230352'

p='docs/dossie/LEIA-ME.md'
s=io.open(p,encoding='utf-8').read()
velho="""`pedidos.html` é a relação de tudo que o Adriano pediu, com o estado de cada
item. Ela **não se edita** — sai de"""
novo=f"""`pedidos.html` é a relação de tudo que o Adriano pediu, com o estado de cada
item, publicada em:

**{URL}**

Publique **passando essa URL**, para cair na mesma página. Ela **não se edita** —
sai de"""
assert s.count(velho)==1
io.open(p,'w',encoding='utf-8').write(s.replace(velho,novo))

p='../CLAUDE.md'
s=io.open(p,encoding='utf-8').read()
velho="""O que falta no projeto está em `phxsql/docs/PENDENCIAS.md` — atualize junto com
o dossiê."""
novo=f"""O que falta no projeto está em `phxsql/docs/PENDENCIAS.md` — atualize junto com
o dossiê.

Dessa lista sai uma **segunda página**, a relação dos pedidos com o estado de
cada um:

- **URL:** {URL}
- **Fonte:** `phxsql/docs/dossie/pedidos.html`, que **não se edita** —
  `python3 phxsql/docs/dossie/pagina-dos-pedidos.py` a gera do `PENDENCIAS.md`
  e conta os três estados sozinho."""
assert s.count(velho)==1
io.open(p,'w',encoding='utf-8').write(s.replace(velho,novo))
print('ok')
