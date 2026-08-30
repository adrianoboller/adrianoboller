# Style the page box
# 28/08 19:50

import pathlib
p = pathlib.Path("crates/phxsql-server/ui/index.html")
s = p.read_text()
antigo = """.paginar{display:inline-flex;gap:6px;margin-left:8px}"""
novo = """.paginar{display:inline-flex;gap:6px;margin-left:8px;align-items:center}
/* A caixa de ir para a pagina: larga o bastante para cinco digitos, porque
   uma tabela com mais de dez mil paginas e exatamente o caso em que ela
   serve. `tabular-nums` para o numero nao dancar enquanto se digita. */
.campo-pagina{width:5.5em;padding:3px 6px;border:1px solid var(--linha);
  border-radius:6px;background:var(--fundo-2);color:var(--texto);
  font:inherit;font-size:12px;text-align:right;font-variant-numeric:tabular-nums}
.campo-pagina:disabled{opacity:.35}"""
assert antigo in s
s = s.replace(antigo, novo)
p.write_text(s)
print("ok")
