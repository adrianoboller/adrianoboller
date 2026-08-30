# Correct the measured numbers
# 27/08 21:24

import subprocess, pathlib
rust = int(subprocess.run("find crates -name '*.rs' | xargs cat | wc -l", shell=True, capture_output=True, text=True).stdout)
doc  = int(subprocess.run("cat docs/*.md README.md MANUAL.txt CHANGELOG.md marca/LEIA-ME.md docs/dossie/LEIA-ME.md | wc -l", shell=True, capture_output=True, text=True).stdout)
ui   = pathlib.Path('crates/phxsql-server/ui/index.html').stat().st_size
p = pathlib.Path('docs/dossie/dossie-phxsql.html')
s = p.read_text()
s = s.replace('<div><div class="v">19.283</div><div class="r">linhas de Rust</div></div>',
              f'<div><div class="v">{rust:,}</div><div class="r">linhas de Rust</div></div>'.replace(',', '.'))
s = s.replace('<div><div class="v">2.901</div><div class="r">linhas de doc</div></div>',
              f'<div><div class="v">{doc:,}</div><div class="r">linhas de doc</div></div>'.replace(',', '.'))
s = s.replace('PhxSql 0.3.0 · 19.283 linhas de Rust em 4 crates, mais 69 KB de interface',
              f'PhxSql 0.3.0 · {rust:,} linhas de Rust em 4 crates, mais {round(ui/1024)} KB de interface'.replace(',', '.'))
p.write_text(s)
print(f'medido: {rust} linhas de Rust, {doc} de doc, interface {round(ui/1024)} KB')
