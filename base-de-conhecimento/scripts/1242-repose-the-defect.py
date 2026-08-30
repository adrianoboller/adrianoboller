# Repose the defect
# 30/08 04:20

import io
p="phxsql/crates/phxsql-server/ui/index.html"
s=io.open(p,encoding="utf-8").read()
# defeito reposto: a guarda deixa de guardar
s=s.replace("  const aindaEMinha = () => minhaVez === admGeracao;",
            "  const aindaEMinha = () => true;  // DEFEITO REPOSTO",1)
io.open(p,"w",encoding="utf-8").write(s)
