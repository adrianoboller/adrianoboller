# Start the clock at boot
# 28/08 13:50

import pathlib
p = pathlib.Path('crates/phxsql-server/src/servidor.rs')
s = p.read_text()
v = 'self.subir_backup_agendado();'
assert s.count(v) == 1
p.write_text(s.replace(v, 'self.subir_backup_agendado();\n        self.ligar_relogio_de_gravacao();'))
print('relogio ligado no arranque')
