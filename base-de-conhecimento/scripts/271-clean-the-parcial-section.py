# Clean the parcial section
# 28/08 11:02

import pathlib
p = pathlib.Path('docs/PENDENCIAS.md')
s = p.read_text()

v = '''## 2. Parcial

Existe, funciona no que promete, mas **não faz tudo** o que o pedido queria.
Cada linha diz exatamente onde para.
'''
n = '''## 2. Parcial

Existe, funciona no que promete, mas **não faz tudo** o que o pedido queria.
Cada linha diz exatamente onde para.

As duas primeiras são os dois ◐ da tabela lá em cima. A terceira **não é um
pedido seu** — é um buraco achado na revisão, dentro de um pedido marcado
feito, e fica aqui para não sumir de vista.
'''
assert s.count(v) == 1
s = s.replace(v, n)

# gráficos: resolvido -- a página está versionada
v = '''| 3 | **Gráficos comparativos** de IO, memória e CPU | `bancada/graficos.py` gera a página inteira a partir do `resultados.json` | a página gerada não estava **versionada** — existia só na máquina de quem rodou. Passa a entrar no repositório |
| 4 | **Subir o PhxSql no GitHub** |'''
n = '''| 2 | **Subir o PhxSql no GitHub** |'''
assert s.count(v) == 1
s = s.replace(v, n)

# a chave estrangeira desce para o fim, e numerada como o que e
v = '''| 2 | **Chave estrangeira** com CASCADE / RESTRICT / SET NULL |'''
n = '''| 3 | **Chave estrangeira** com CASCADE / RESTRICT / SET NULL |'''
assert s.count(v) == 1
s = s.replace(v, n)
p.write_text(s)
print('ok')
