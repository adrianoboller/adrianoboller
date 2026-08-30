# Bump versions and document the interface-size recipe
# 28/08 11:06

import pathlib
p = pathlib.Path('docs/dossie/LEIA-ME.md')
s = p.read_text()
v = '''cat docs/*.md README.md CHANGELOG.md MANUAL.txt \\
    bancada/LEIA-ME.md marca/LEIA-ME.md docs/dossie/LEIA-ME.md \\
  | wc -l                                                          # linhas de doc
```'''
n = '''cat docs/*.md README.md CHANGELOG.md MANUAL.txt \\
    bancada/LEIA-ME.md marca/LEIA-ME.md docs/dossie/LEIA-ME.md \\
  | wc -l                                                          # linhas de doc
stat -c%s crates/phxsql-server/ui/index.html \\
          crates/phxsql-server/ui/grid/phx-grid.{css,js} \\
  | paste -sd+ | bc                                                # bytes de interface
```

A interface são os **três arquivos que o `http.rs` embute com `include_str!`** —
`index.html` mais o CSS e o JS do phx-grid. Contar só o `index.html` daria um
número menor do que o publicado, e ninguém conseguiria reproduzir o rodapé.'''
assert s.count(v) == 1
p.write_text(s.replace(v, n))
print('ok')
