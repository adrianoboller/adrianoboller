# Avoid stacking listeners
# 28/08 11:33

import pathlib
p = pathlib.Path('crates/phxsql-server/ui/index.html')
s = p.read_text()
# ligarMenu registra um listener; chamar de novo empilharia. O listener e
# delegado no proprio #menubar, entao refazer o innerHTML basta.
s = s.replace('    montarMenu(); montarFerramentas(); ligarMenu();\n    avisar(`${Object.keys(novo).length} rótulo(s) trocado(s)`);',
              '    montarMenu(); montarFerramentas();\n    avisar(`${Object.keys(novo).length} rótulo(s) trocado(s)`);')
s = s.replace('    montarMenu(); montarFerramentas(); ligarMenu();\n    avisar("nomes de fábrica de volta");',
              '    montarMenu(); montarFerramentas();\n    avisar("nomes de fábrica de volta");')
s = s.replace('  const [u, c] = await Promise.all([api("usuarios"), api("config")]);\n  const lista = u.usuarios || u || [];',
              '  const u = await api("usuarios");\n  const lista = u.usuarios || u || [];')
p.write_text(s)
print('ok')
