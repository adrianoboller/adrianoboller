# Fix CAMPOS_CONHECIDOS array size
# 29/08 17:15

import pathlib, re
p = pathlib.Path("crates/phxsql-server/src/config.rs")
t = p.read_text()
m = re.search(r"const CAMPOS_CONHECIDOS: \[&str; (\d+)\] = \[(.*?)\];", t, re.S)
decl, corpo = int(m.group(1)), m.group(2)
reais = len(re.findall(r'"', corpo)) // 2
print(f"declarado {decl}, campos reais {reais}")
if decl != reais:
    t = t[:m.start(1)] + str(reais) + t[m.end(1):]
    p.write_text(t)
    print(f"corrigido para {reais}")
