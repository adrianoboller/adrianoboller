# Add the missing entry and look for a reserved-word list
# 29/08 03:04

import pathlib
p = pathlib.Path("crates/phxsql-server/ui/index.html")
s = p.read_text()
s = s.replace('''  cpu_percentual:     ["quantos núcleos o trabalho dividido usa", "não é cota do sistema operacional"],''',
'''  cpu_percentual:     ["quantos núcleos o trabalho dividido usa", "não é cota do sistema operacional"],
  nucleos_efetivos:   ["quantos núcleos isso dá nesta máquina", "calculado, não configurado"],''',1)
p.write_text(s)
