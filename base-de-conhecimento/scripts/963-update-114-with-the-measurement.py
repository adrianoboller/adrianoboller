# Update #114 with the measurement
# 29/08 00:56

import pathlib
p = pathlib.Path("docs/PENDENCIAS.md")
s = p.read_text()
alvo = '''| ☐ | 114 | **Índice não único fora do caminho crítico** | 1,45× medido, e não custa correção: nada depende de um índice não único para decidir se a linha entra. O **único** não pode sair — ele é a própria decisão, e adiá-lo deixaria buraco permanente no `.reg` |'''
novo = '''| ☐ | 114 | **Índice não único fora do caminho crítico** | **remedido, e o alvo mudou outra vez.** Adiar o índice e reconstruir no fim vale **1,02×**, e não 1,45×: o `reindexar` de hoje insere **chave a chave**, uma descida por chave — exatamente o trabalho que se queria evitar. O ganho está na **reconstrução em lote** (varrer, ordenar, encher as folhas em sequência), cujo piso medido é **0,21 s de varredura + 0,03 s de ordenação** contra os 2,54 s que o `reindexar` cobra. Faça o lote primeiro; adiar sozinho não compra nada. `--example indice-adiado` |'''
assert s.count(alvo) == 1
s = s.replace(alvo, novo, 1)
p.write_text(s)
print("ok")
