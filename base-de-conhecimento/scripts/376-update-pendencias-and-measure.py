# Update PENDENCIAS and measure
# 28/08 13:36

import pathlib
p = pathlib.Path('docs/PENDENCIAS.md'); s = p.read_text()
v = '''| ☑️ | 67 | **Botão e menu Tabelas**'''
n = '''| ☑️ | 77 | **Group dinâmico pelas colunas na grade**, como o Janus GridEX(R) e o DevExpress(R) | já havia arrastar e multinível; entraram ordem por nível, rodapé por grupo com o total na coluna, total geral e expandir/recolher tudo |
| ☑️ | 78 | **Botão que monta pivot dinâmico com assistente**, pedindo as tabelas envolvidas | operação `pivotar` no servidor com *hash join*, seis resumos e granularidade de data; assistente de três passos na tela |
| ☑️ | 67 | **Botão e menu Tabelas**'''
assert s.count(v) == 1
s = s.replace(v, n)
v = '''**68 feitos · 2 parciais · 6 planejados**, de 76 pedidos.'''
n = '''**70 feitos · 2 parciais · 6 planejados**, de 78 pedidos.'''
assert s.count(v) == 1
p.write_text(s.replace(v, n))
print('PENDENCIAS')
