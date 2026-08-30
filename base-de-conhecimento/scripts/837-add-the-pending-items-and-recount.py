# Add the pending items and recount
# 28/08 21:26

import pathlib
p = pathlib.Path("docs/PENDENCIAS.md")
s = p.read_text()
antigo = """| ☑️ | 67 | **Botão e menu Tabelas**"""
novo = """| ☑️ | 112 | **Analisar as sugestões de arquitetura (WAL, MemTable, group commit, LSM)** | `docs/DESEMPENHO.md`, com a medição que muda o alvo: **83,5% do tempo de uma inserção está no `.ndx`**, e o arquivo de dados — o que as propostas querem substituir — já é *append-only* e custa 16,5%. Das dez propostas, cinco já existem, duas miram um gargalo que não é o nosso, uma quebraria a ordem de digitação, e duas são reais |
| ☐ | 113 | **Ordenar as chaves do lote antes de inserir no `.ndx`** | o item que a medição favorece: ataca os 83,5% sem mudar formato nem garantia. A carga em lote já provou o princípio no nível de cima (9,6×); falta aplicá-lo dentro da B+tree |
| ☐ | 114 | **Índice não único fora do caminho crítico** | 1,45× medido, e não custa correção: nada depende de um índice não único para decidir se a linha entra. O **único** não pode sair — ele é a própria decisão, e adiá-lo deixaria buraco permanente no `.reg` |
| ☑️ | 67 | **Botão e menu Tabelas**"""
assert antigo in s
s = s.replace(antigo, novo)
p.write_text(s)
