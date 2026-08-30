# Remove duplicate binding and run all gates
# 29/08 18:11

import pathlib
p = pathlib.Path("crates/phxsql-server/src/servidor.rs"); t = p.read_text()
# Duas copias do mesmo valor, uma de cada frente. Fica a tripla, que serve aos
# tres campos vivos; a copia solta sai.
velho = "        let somente_leitura = config.somente_leitura;\n"
assert velho in t
p.write_text(t.replace(velho, "", 1)); print("copia duplicada removida")
