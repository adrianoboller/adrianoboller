# Update #113 in PENDENCIAS
# 29/08 00:22

import pathlib
p = pathlib.Path("docs/PENDENCIAS.md")
s = p.read_text()
alvo = '''| ☐ | 113 | **Ordenar as chaves do lote antes de inserir no `.ndx`** | o item que a medição favorece: ataca os 83,5% sem mudar formato nem garantia. A carga em lote já provou o princípio no nível de cima (9,6×); falta aplicá-lo dentro da B+tree |'''
novo = '''| ◐ | 113 | **Atacar os 83,5% do `.ndx`** | **medido, e o alvo era outro.** O custo não era de localidade de chave — era **reler e recalcular CRC-32 da mesma página**: 10,86 páginas tocadas por linha, 8,80 delas releituras, 2,34 µs de CRC cada. Um **cache de páginas de leitura** no `.ndx` levou a inserção de **44,4 → 18,5 µs (2,40×)** e a carga em lote pela rede de **25.985 → 37.021 linhas/s**. Ordenar as chaves em si valia 1,06× antes do cache e vale **1,19×** depois — está medido em `docs/DESEMPENHO.md` §4.1, com a garantia que teria de ser rebaixada para implementá-lo |'''
assert s.count(alvo) == 1
s = s.replace(alvo, novo, 1)
s = s.replace("**108 feitos · 7 parciais · 12 planejados**, de 127 pedidos.",
              "**108 feitos · 8 parciais · 11 planejados**, de 127 pedidos.", 1)
p.write_text(s)
print("ok")
