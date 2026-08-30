# Replace messy region with intact modules from each side
# 29/08 17:24

import pathlib
p = pathlib.Path("phxsql/crates/phxsql-server/src/servidor.rs")
linhas = p.read_text().splitlines(keepends=True)
fw = pathlib.Path("/tmp/mod_firewall.rs").read_text()
pa = pathlib.Path("/tmp/mod_papel.rs").read_text()
# troca a regiao 10899..11439 (1-based, inclusive) pelos dois modulos inteiros
novo = linhas[:10898] + [fw, "\n", pa, "\n"] + linhas[11439:]
p.write_text("".join(novo))
print("regiao substituida")
