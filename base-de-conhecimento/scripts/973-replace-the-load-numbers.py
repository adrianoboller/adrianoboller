# Replace the load numbers
# 29/08 01:13

import pathlib
alvos = {
 "docs/DESEMPENHO.md": [("2.609), que é o controle; o lote subiu de 25.985 para 37.021 por causa do cache",
                        "2.659), que é o controle; o lote subiu de 25.985 para 39.287 por causa do cache")],
 "README.md": [("**2.609 → 37.021 linhas/s\n(14,2×)**", "**2.659 → 39.287 linhas/s\n(14,8×)**")],
 "docs/dossie/dossie-phxsql-0.15.html": [("<strong>2.609 → 37.021 linhas/s, 14,2×</strong>",
                                          "<strong>2.659 → 39.287 linhas/s, 14,8×</strong>")],
}
for f, pares in alvos.items():
    p = pathlib.Path(f); s = p.read_text()
    for a, b in pares:
        if a not in s:
            print("NAO ACHOU em", f, repr(a[:50])); continue
        s = s.replace(a, b)
    p.write_text(s)
print("ok")
