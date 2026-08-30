# Add conflict dialog and CSS
# 28/08 23:56

import pathlib
p = pathlib.Path("crates/phxsql-server/ui/index.html")
s = p.read_text()
js = pathlib.Path("/tmp/claude-0/-home-user-adrianoboller/34595649-0af6-575a-8f79-80dbe8cb7a5d/scratchpad/conflito.js").read_text()
alvo = '''/** Restaura uma linha marcada. */'''
assert s.count(alvo) == 1
s = s.replace(alvo, js + alvo, 1)

# CSS
alvo = '''.marca-excluida{color:var(--log);font-weight:600}'''
novo = '''.marca-excluida{color:var(--log);font-weight:600}

/* A caixa do conflito de escrita e mais larga que as outras de proposito: sao
   tres valores lado a lado, e espremer a comparacao em 560px derrotaria o
   unico motivo de ela existir. */
.caixa.larga{max-width:880px}
table.conf td,table.conf th{padding:5px 9px;font-size:12px}
table.conf .col{font-weight:600;white-space:nowrap}
table.conf tr.diverge td{background:rgba(255,196,61,.06)}
table.conf .esc{display:flex;gap:7px;align-items:baseline;cursor:pointer;margin:0}
table.conf .esc input{margin:0;flex:0 0 auto}
.vazio-nulo{color:var(--texto-3);font-style:italic}'''
assert s.count(alvo) == 1
s = s.replace(alvo, novo, 1)
p.write_text(s)
print("ok")
