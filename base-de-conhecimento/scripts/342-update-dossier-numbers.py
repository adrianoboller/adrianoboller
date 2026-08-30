# Update dossier numbers
# 28/08 11:51

import pathlib
p = pathlib.Path('docs/dossie/dossie-phxsql.html')
s = p.read_text()
trocas = [
 ('<div class="selo">Dossiê técnico · versão 0.7.0</div>',
  '<div class="selo">Dossiê técnico · versão 0.8.0</div>'),
 ('<div><div class="v">23.226</div><div class="r">linhas de Rust</div></div>',
  '<div><div class="v">24.711</div><div class="r">linhas de Rust</div></div>'),
 ('<div><div class="v">339</div><div class="r">testes</div></div>',
  '<div><div class="v">355</div><div class="r">testes</div></div>'),
 ('<div><div class="v">4.290</div><div class="r">linhas de doc</div></div>',
  '<div><div class="v">4.612</div><div class="r">linhas de doc</div></div>'),
 ('<p>PhxSql 0.7.0 · 23.226 linhas de Rust em 4 crates, mais 263 KiB de interface ·',
  '<p>PhxSql 0.8.0 · 24.711 linhas de Rust em 4 crates, mais 323 KiB de interface ·'),
 ('  339 testes · nenhuma dependência externa.',
  '  355 testes · nenhuma dependência externa.'),
 ('<tr><td class="dado">criar</td><td>criar_database, criar_schema, criar_tabela, duplicar_tabela</td></tr>',
  '<tr><td class="dado">criar</td><td>criar_database, criar_schema, criar_tabela, duplicar_tabela, copiar_tabela</td></tr>'),
 ('<tr><td>Gestão de tabelas · criar, duplicar, reparar, partições e excluir</td><td><span class="pino ok">pronto</span></td><td class="num">14</td></tr>',
  '<tr><td>Gestão de tabelas · criar, duplicar, reparar, partições e excluir</td><td><span class="pino ok">pronto</span></td><td class="num">14</td></tr>\n'
  '        <tr><td>Partição por período · mensal, bimestral, semestral, anual</td><td><span class="pino ok">pronto</span></td><td class="num">6</td></tr>\n'
  '        <tr><td>Metadados de campo · id estável, caption, descrição, máscara</td><td><span class="pino ok">pronto</span></td><td class="num">7</td></tr>\n'
  '        <tr><td>Chave primária declarada · composta derivada dos índices</td><td><span class="pino ok">pronto</span></td><td class="num">—</td></tr>\n'
  '        <tr><td>SysTables e SysColumns · catálogo e dicionário de dados</td><td><span class="pino ok">pronto</span></td><td class="num">—</td></tr>\n'
  '        <tr><td>Copiar e colar tabela entre bancos e schemas</td><td><span class="pino ok">pronto</span></td><td class="num">3</td></tr>\n'
  '        <tr><td>Gerir banco · configurações, diretivas, conexões, backup</td><td><span class="pino ok">pronto</span></td><td class="num">—</td></tr>\n'
  '        <tr><td>Editor de menu · troca o nome exibido de cada item</td><td><span class="pino ok">pronto</span></td><td class="num">—</td></tr>'),
 ('<tr><td>Barra de ferramentas · 17 ferramentas, 13 vivas e 4 dizendo o que falta</td><td><span class="pino ok">pronto</span></td><td class="num">—</td></tr>',
  '<tr><td>Barra de ferramentas · 20 ferramentas, 16 vivas e 4 dizendo o que falta</td><td><span class="pino ok">pronto</span></td><td class="num">—</td></tr>'),
 ('<tr><td>Barra de menu tradicional · sete menus · Alt, setas e Esc</td><td><span class="pino ok">pronto</span></td><td class="num">—</td></tr>',
  '<tr><td>Barra de menu tradicional · nove menus · Alt, setas e Esc</td><td><span class="pino ok">pronto</span></td><td class="num">—</td></tr>'),
 ('<tr><td>Triggers nas três operações</td><td><span class="pino nao">a fazer</span></td><td class="num">—</td></tr>',
  '<tr><td>Triggers nas três operações · tela apagada diz o que falta</td><td><span class="pino nao">a fazer</span></td><td class="num">—</td></tr>'),
]
for v, n in trocas:
    assert s.count(v) == 1, v[:70]
    s = s.replace(v, n)
p.write_text(s)
print('dossie: numeros e quadro de estado')
