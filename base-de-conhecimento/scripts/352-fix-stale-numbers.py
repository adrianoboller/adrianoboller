# Fix stale numbers
# 28/08 13:11

import pathlib
p = pathlib.Path('docs/dossie/dossie-phxsql.html')
s = p.read_text()
trocas = [
 # a CLI tem 11 comandos, nao 9 -- conferido no `phxsql --help`
 ('<text x="515" y="156" text-anchor="middle" font-size="10" opacity=".5">9 comandos</text>',
  '<text x="515" y="156" text-anchor="middle" font-size="10" opacity=".5">11 comandos</text>'),
 ('<tr><td>Linha de comando · 9 comandos</td><td><span class="pino ok">pronto</span></td><td class="num">—</td></tr>',
  '<tr><td>Linha de comando · 11 comandos</td><td><span class="pino ok">pronto</span></td><td class="num">—</td></tr>'),
 ('<tr><td>View Database · grade de tabelas e ficha de edição · 30 das 33 ops na tela</td><td><span class="pino ok">pronto</span></td><td class="num">—</td></tr>',
  '<tr><td>View Database · grade de tabelas e ficha de edição · 33 das 36 ops na tela</td><td><span class="pino ok">pronto</span></td><td class="num">—</td></tr>'),
 ('<tr><td>Gestão de tabelas · criar, duplicar, reparar, partições e excluir</td><td><span class="pino ok">pronto</span></td><td class="num">14</td></tr>',
  '<tr><td>Gestão de tabelas · onze operações sobre a tabela escolhida</td><td><span class="pino ok">pronto</span></td><td class="num">14</td></tr>'),
]
for v, n in trocas:
    assert s.count(v) == 1, v[:60]
    s = s.replace(v, n)
p.write_text(s)
print('numeros defasados corrigidos')
