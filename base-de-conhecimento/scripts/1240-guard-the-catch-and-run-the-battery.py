# Guard the catch and run the battery
# 30/08 04:13

import io
p="phxsql/crates/phxsql-server/ui/index.html"
linhas=io.open(p,encoding="utf-8").read().split("\n")
i=3542  # linha 3543, 1-based: o catch do abrirAdmin
assert linhas[i].strip().startswith('p.innerHTML = `<div class="aviso mal">'), linhas[i]
linhas[i:i+1] = ["    // Recado de erro tambem nao pinta por cima da tela seguinte.",
                 "    if (!aindaEMinha()) return;",
                 linhas[i]]
io.open(p,"w",encoding="utf-8").write("\n".join(linhas))
print("catch guardado")
