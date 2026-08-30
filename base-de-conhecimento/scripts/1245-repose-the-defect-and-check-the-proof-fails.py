# Repose the defect and check the proof fails
# 30/08 04:24

import io
p="phxsql/crates/phxsql-server/ui/index.html"
s=io.open(p,encoding="utf-8").read()
velho="  const aindaEMinha = () => minhaVez === admGeracao;"
assert s.count(velho)==1
io.open(p,"w",encoding="utf-8").write(s.replace(velho,"  const aindaEMinha = () => true; // DEFEITO REPOSTO",1))
print("defeito reposto")
