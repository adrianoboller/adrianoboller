# Tidy state and check ligarMenu
# 28/08 11:33

import pathlib
p = pathlib.Path('crates/phxsql-server/ui/index.html')
s = p.read_text()
s = s.replace('folha(`Gestão de ${tab}`, `${db} · oito operações sobre esta tabela`,',
              'folha(`Gestão de ${tab}`, `${db} · o que se faz com esta tabela`,')
s = s.replace('              rascunho:null };',
              '''              rascunho:null,
              // A area de transferencia do copiar/colar de tabela, e os
              // rotulos que o editor de menu trocou.
              copia:null, rotulos:null };''')
s = s.replace('  const _ = { bancos, c };\n', '')
s = s.replace('  const bancos = est.bancos || [];\n\n  folha("Configurações dos usuários"',
              '  folha("Configurações dos usuários"')
p.write_text(s)
print('ok')
