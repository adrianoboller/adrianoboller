# Document in MANUAL and PENDENCIAS
# 29/08 03:00

import pathlib
p = pathlib.Path("docs/PENDENCIAS.md")
s = p.read_text()
alvo = '''**109 feitos · 8 parciais · 10 planejados**, de 127 pedidos.'''
novo = '''**110 feitos · 8 parciais · 10 planejados**, de 128 pedidos.'''
assert s.count(alvo) == 1
s = s.replace(alvo, novo, 1)
alvo2 = '''| ☐ | 127 | **Diagrama ER e editor de modelo**'''
novo2 = '''| ☑️ | 128 | **`BULKINSERT(true/false)`: a tabela reservada para a carga** | reserva exclusiva por conexão, com erro **4002 `EM_CARGA`** para os outros — nomeando quem reservou e com `repetir: true`, que é o que separa «espere» de «você não pode». **1,53× medido** (43.500 → 66.500 linhas/s), porque reservada a janela de durabilidade não fecha e a carga vira um `fsync` só. Duas redes contra reserva órfã: a queda da conexão solta na hora, e `recursos.carga_prazo_min` solta o soquete pendurado. Só pela porta de dados. 10 testes mais a prova pelo soquete em `bancada/carga/bulkinsert.py` |
| ☐ | 127 | **Diagrama ER e editor de modelo**'''
assert s.count(alvo2) == 1
s = s.replace(alvo2, novo2, 1)
p.write_text(s)
