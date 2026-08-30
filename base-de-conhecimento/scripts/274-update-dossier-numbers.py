# Update dossier numbers
# 28/08 11:03

import pathlib
p = pathlib.Path('docs/dossie/dossie-phxsql.html')
s = p.read_text()

trocas = [
    # capa
    ('<div><div class="v">22.432</div><div class="r">linhas de Rust</div></div>',
     '<div><div class="v">23.226</div><div class="r">linhas de Rust</div></div>'),
    ('<div><div class="v">324</div><div class="r">testes</div></div>',
     '<div><div class="v">339</div><div class="r">testes</div></div>'),
    # rodape
    ('<p>PhxSql 0.6.0 · 22.432 linhas de Rust em 4 crates, mais 232 KiB de interface ·',
     '<p>PhxSql 0.6.0 · 23.226 linhas de Rust em 4 crates, mais 264 KiB de interface ·'),
    ('  324 testes · nenhuma dependência externa.',
     '  339 testes · nenhuma dependência externa.'),
    # permissoes
    ('<tr><td class="dado">criar</td><td>criar_database, criar_schema</td></tr>',
     '<tr><td class="dado">criar</td><td>criar_database, criar_schema, criar_tabela, duplicar_tabela</td></tr>'),
    # quadro de estado
    ('<tr><td>View Database · grade de tabelas e ficha de edição · 30 das 32 ops na tela</td><td><span class="pino ok">pronto</span></td><td class="num">—</td></tr>',
     '<tr><td>View Database · grade de tabelas e ficha de edição · 30 das 33 ops na tela</td><td><span class="pino ok">pronto</span></td><td class="num">—</td></tr>\n'
     '        <tr><td>Gestão de tabelas · criar, duplicar, reparar, partições e excluir</td><td><span class="pino ok">pronto</span></td><td class="num">14</td></tr>'),
    ('<tr><td>Barra de ferramentas · 15 ferramentas, 10 vivas e 5 dizendo o que falta</td><td><span class="pino ok">pronto</span></td><td class="num">—</td></tr>',
     '<tr><td>Barra de ferramentas · 17 ferramentas, 13 vivas e 4 dizendo o que falta</td><td><span class="pino ok">pronto</span></td><td class="num">—</td></tr>'),
    ('<tr><td>Barra de menu tradicional · 22 recursos · Alt, setas e Esc</td><td><span class="pino ok">pronto</span></td><td class="num">—</td></tr>',
     '<tr><td>Barra de menu tradicional · sete menus · Alt, setas e Esc</td><td><span class="pino ok">pronto</span></td><td class="num">—</td></tr>'),
]
for v, n in trocas:
    assert s.count(v) == 1, f'nao achei (ou achei demais): {v[:70]}'
    s = s.replace(v, n)
p.write_text(s)
print('ok')
