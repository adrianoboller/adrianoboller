# Verify the profiler screen in a browser
# 28/08 22:59

import pathlib
p = pathlib.Path("crates/phxsql-server/ui/index.html")
s = p.read_text()
antigo = """.cores-acao i{width:10px;height:10px;border-radius:3px;display:inline-block}"""
novo = """.cores-acao i{width:10px;height:10px;border-radius:3px;display:inline-block}

/* --------------------------------- profiler --------------------------- */
.prof-topo{flex-wrap:wrap;align-items:flex-end;gap:12px}
.mini-campo{display:flex;flex-direction:column;gap:4px;font-size:10px;
  letter-spacing:.1em;text-transform:uppercase;color:var(--texto-3)}
.mini-campo input,.mini-campo select{padding:6px 8px;font-size:12.5px;
  border:1px solid var(--linha);border-radius:6px;background:var(--painel-2);
  color:var(--texto);font-family:inherit;min-width:140px}
.mini-campo.largo input{min-width:320px}
.mini-campo.caixa{flex-direction:row;align-items:center;gap:6px;text-transform:none;
  letter-spacing:0;font-size:12px;padding-bottom:8px}
.mini-campo.caixa input{min-width:0}
table.prof{font-size:12.5px}
table.prof td,table.prof th{padding:5px 10px}
/* O pedido e o unico campo de tamanho livre: ele corta com reticencias e o
   texto inteiro fica no `title`. Sem o corte, uma insercao grande empurraria
   a tabela inteira para fora da tela. */
.prof-pedido{max-width:640px;overflow:hidden;text-overflow:ellipsis;
  white-space:nowrap;color:var(--texto-2)}
tr.prof-mal td{background:color-mix(in srgb,var(--log) 8%,transparent)}"""
assert antigo in s
s = s.replace(antigo, novo)
p.write_text(s)
