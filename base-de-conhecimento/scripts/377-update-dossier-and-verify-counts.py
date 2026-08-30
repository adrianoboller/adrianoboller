# Update dossier and verify counts
# 28/08 13:37

import pathlib
p = pathlib.Path('docs/dossie/dossie-phxsql.html'); s = p.read_text()
trocas = [
 ('<div class="selo">Dossiê técnico · versão 0.8.0</div>',
  '<div class="selo">Dossiê técnico · versão 0.9.0</div>'),
 ('<div><div class="v">24.711</div><div class="r">linhas de Rust</div></div>',
  '<div><div class="v">25.768</div><div class="r">linhas de Rust</div></div>'),
 ('<div><div class="v">355</div><div class="r">testes</div></div>',
  '<div><div class="v">367</div><div class="r">testes</div></div>'),
 ('<div><div class="v">4.656</div><div class="r">linhas de doc</div></div>',
  '<div><div class="v">4.770</div><div class="r">linhas de doc</div></div>'),
 ('<p>PhxSql 0.8.0 · 24.711 linhas de Rust em 4 crates, mais 323 KiB de interface ·',
  '<p>PhxSql 0.9.0 · 25.768 linhas de Rust em 4 crates, mais 352 KiB de interface ·'),
 ('  355 testes · nenhuma dependência externa.',
  '  367 testes · nenhuma dependência externa.'),
 ('<tr><td>phx-grid na aba Conteúdo · agrupamento por arrastar</td><td><span class="pino ok">pronto</span></td><td class="num">—</td></tr>',
  '<tr><td>phx-grid · agrupamento por arrastar, ordem por nível, rodapé e total geral</td><td><span class="pino ok">pronto</span></td><td class="num">—</td></tr>\n'
  '        <tr><td>Tabela dinâmica · cruzamento somado no servidor, com <em>hash join</em></td><td><span class="pino ok">pronto</span></td><td class="num">12</td></tr>'),
 ('<p><strong>33 das 36 operações do protocolo têm tela.</strong>',
  '<p><strong>34 das 37 operações do protocolo têm tela.</strong>'),
 ('<tr><td>Barra de ferramentas · 20 ferramentas, 16 vivas e 4 dizendo o que falta</td><td><span class="pino ok">pronto</span></td><td class="num">—</td></tr>',
  '<tr><td>Barra de ferramentas · 21 ferramentas, 17 vivas e 4 dizendo o que falta</td><td><span class="pino ok">pronto</span></td><td class="num">—</td></tr>'),
]
for v, n in trocas:
    assert s.count(v) == 1, v[:60]
    s = s.replace(v, n)
p.write_text(s)
print('dossie: numeros')
