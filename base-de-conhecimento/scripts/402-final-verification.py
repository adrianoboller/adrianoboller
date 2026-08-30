# Final verification
# 28/08 14:10

import pathlib
p = pathlib.Path('docs/dossie/dossie-phxsql.html'); s = p.read_text()
v = '<div><div class="v">4.955</div><div class="r">linhas de doc</div></div>'
n = '<div><div class="v">$(printf '%s' $DOC | sed 's/\(.\)\(...\)$/\1.\2/')</div><div class="r">linhas de doc</div></div>'
assert s.count(v) == 1
p.write_text(s.replace(v, n)); print('doc: $DOC')
