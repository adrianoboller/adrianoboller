# Remeasure and update the dossier numbers
# 27/08 22:44

import subprocess, pathlib
rust = int(subprocess.run("find crates -name '*.rs' | xargs cat | wc -l", shell=True, capture_output=True, text=True).stdout)
doc  = int(subprocess.run("cat docs/*.md README.md MANUAL.txt CHANGELOG.md marca/LEIA-ME.md docs/dossie/LEIA-ME.md bancada/LEIA-ME.md | wc -l", shell=True, capture_output=True, text=True).stdout)
ui   = pathlib.Path('crates/phxsql-server/ui/index.html').stat().st_size
grid = pathlib.Path('crates/phxsql-server/ui/grid/phx-grid.js').stat().st_size + pathlib.Path('crates/phxsql-server/ui/grid/phx-grid.css').stat().st_size

p = pathlib.Path('docs/dossie/dossie-phxsql.html')
s = p.read_text()
s = s.replace('<div><div class="v">19.242</div><div class="r">linhas de Rust</div></div>',
              f'<div><div class="v">{rust:,}</div><div class="r">linhas de Rust</div></div>'.replace(',','.'))
s = s.replace('<div><div class="v">3.024</div><div class="r">linhas de doc</div></div>',
              f'<div><div class="v">{doc:,}</div><div class="r">linhas de doc</div></div>'.replace(',','.'))
s = s.replace('<div class="selo">Dossiê técnico · versão 0.3.0</div>',
              '<div class="selo">Dossiê técnico · versão 0.4.0</div>')
s = s.replace('PhxSql 0.3.0 · 19.242 linhas de Rust em 4 crates, mais 69 KB de interface',
              f'PhxSql 0.4.0 · {rust:,} linhas de Rust em 4 crates, mais {round(ui/1024)} KB de interface e {round(grid/1024)} KB de grid'.replace(',','.'))
p.write_text(s)
print(f'medido: {rust} Rust, {doc} doc, interface {round(ui/1024)} KB, grid {round(grid/1024)} KB')
