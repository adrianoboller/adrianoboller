# Poe a grade e recaptura
# 01/09 18:39

import pathlib
D = pathlib.Path("/tmp/claude-0/-home-user-adrianoboller/34595649-0af6-575a-8f79-80dbe8cb7a5d/scratchpad")
h = pathlib.Path("docs/dossie/dossie-phxsql-0.18.html").read_text(encoding="utf-8")
raiz = h[h.index(":root{"):h.index("}", h.index("--lateral:"))+1]
i = h.index("<!-- trio:inicio"); j = h.index("<!-- trio:fim -->")
bloco = h[i:j]
base = """body{margin:0;padding:28px;background:var(--papel);color:var(--tinta);
font:15px/1.65 system-ui,sans-serif;max-width:1000px}
h3{font-size:19px;margin:0 0 8px} .legenda{color:var(--tinta-2);font-size:13px}
.tab{border-collapse:collapse;width:100%;margin:14px 0;font-size:14px}
.tab th,.tab td{border-bottom:1px solid var(--linha);padding:7px 10px;text-align:left}
.tab .num{text-align:right;font-variant-numeric:tabular-nums}
.tab .destaque{font-weight:700;color:var(--acento)}
ul{padding-left:20px} li{margin-bottom:6px;color:var(--tinta-2)}"""
(D/"trio-dossie-claro.html").write_text(
    f"<!doctype html><meta charset=utf-8><style>{raiz}\n{base}</style>{bloco}", encoding="utf-8")
