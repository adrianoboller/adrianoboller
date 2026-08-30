# Update dossier numbers
# 28/08 14:08

import pathlib
p = pathlib.Path('docs/dossie/dossie-phxsql.html'); s = p.read_text()
def troca(v, n):
    global s
    assert s.count(v) == 1, v[:60]
    s = s.replace(v, n)
troca('<div class="selo">Dossiê técnico · versão 0.9.0</div>', '<div class="selo">Dossiê técnico · versão 0.10.0</div>')
troca('<div><div class="v">25.768</div><div class="r">linhas de Rust</div></div>',
      '<div><div class="v">$(printf '%s' $(find . -name '*.rs' -not -path './target/*' | xargs cat | wc -l) | sed 's/\(.\)\(...\)$/\1.\2/')</div><div class="r">linhas de Rust</div></div>')
troca('<div><div class="v">367</div><div class="r">testes</div></div>', '<div><div class="v">375</div><div class="r">testes</div></div>')
troca('<div><div class="v">4.770</div><div class="r">linhas de doc</div></div>',
      '<div><div class="v">$(printf '%s' $DOC | sed 's/\(.\)\(...\)$/\1.\2/')</div><div class="r">linhas de doc</div></div>')
troca('  367 testes · nenhuma dependência externa.', '  375 testes · nenhuma dependência externa.')
troca('<p><strong>34 das 37 operações do protocolo têm tela.</strong>',
      '<p><strong>34 das $OPS operações do protocolo têm tela.</strong>')
p.write_text(s)
print('dossie: numeros')
