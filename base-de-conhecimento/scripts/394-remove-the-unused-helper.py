# Remove the unused helper
# 28/08 13:57

import pathlib
p = pathlib.Path('crates/phxsql-server/src/servidor.rs')
s = p.read_text()
i = s.index('''    /// Roda um trecho com a tabela aberta, do abrir ao fim, sob UMA trava.''')
j = s.index('''    /// Roda um trecho''')
fim = s.index('\n    }\n', s.index('fn com_tabela')) + len('\n    }\n')
s = s[:i] + s[fim:]
p.write_text(s)
print('com_tabela removido -- o padrao explicito ficou mais claro')
