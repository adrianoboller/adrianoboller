# Fix the mangled strings
# 28/08 20:27

import pathlib, re
for f in ["crates/phxsql-store/src/table.rs", "crates/phxsql-server/src/servidor.rs",
          "crates/phxsql-server/src/replica.rs", "crates/phxsql-server/src/usuarios.rs",
          "crates/phxsql-store/src/log.rs", "crates/phxsql-server/src/config.rs"]:
    p = pathlib.Path(f)
    s = p.read_text()
    linhas = s.split("\n")
    mudou = 0
    for i, l in enumerate(linhas):
        if l.lstrip().startswith("//"):
            continue
        # runs de 5+ espacos no meio de um literal, mas nao em tabelas de formato
        novo = re.sub(r'(?<=[a-z,.`)])[ ]{6,}(?=[a-z`])', ' ', l)
        if novo != l:
            linhas[i] = novo
            mudou += 1
    if mudou:
        p.write_text("\n".join(linhas))
        print(f, mudou, "linha(s)")
