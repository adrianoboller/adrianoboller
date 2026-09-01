# Renderiza o bloco do trio nos dois temas
# 01/09 18:38

import re, pathlib
D = pathlib.Path("/tmp/claude-0/-home-user-adrianoboller/34595649-0af6-575a-8f79-80dbe8cb7a5d/scratchpad")
h = pathlib.Path("docs/dossie/dossie-phxsql-0.18.html").read_text(encoding="utf-8")
# O :root do dossie e o bloco do trio, para provar o trio COM as cores dele.
raiz = h[h.index(":root{"):h.index("}", h.index("--lateral:"))+1]
i = h.index("<!-- trio:inicio"); j = h.index("<!-- trio:fim -->")
bloco = h[i:j]
esc = h[h.index("@media (prefers-color-scheme:dark)"):]
esc = esc[:esc.index("\n}")+2]
base = """body{margin:0;padding:28px;background:var(--papel);color:var(--tinta);
font:15px/1.65 system-ui,sans-serif;max-width:1000px}
h3{font-size:19px;margin:0 0 8px} .legenda{color:var(--tinta-2);font-size:13px}
.tab{border-collapse:collapse;width:100%;margin:14px 0;font-size:14px}
.tab th,.tab td{border-bottom:1px solid var(--linha);padding:7px 10px;text-align:left}
.tab .num{text-align:right;font-variant-numeric:tabular-nums}
.tab .destaque{font-weight:700;color:var(--acento)}
ul{padding-left:20px} li{margin-bottom:6px;color:var(--tinta-2)}"""
for tema, extra in (("claro", ""), ("escuro", '<script>document.documentElement.dataset.theme="dark"</script>')):
    forca = esc.replace("@media (prefers-color-scheme:dark)", "@media all") if tema=="escuro" else ""
    pg = f"<!doctype html><meta charset=utf-8><style>{raiz}\n{forca}\n{base}</style>{bloco}{extra}"
    (D / f"trio-dossie-{tema}.html").write_text(pg, encoding="utf-8")
print("duas paginas de prova escritas")
