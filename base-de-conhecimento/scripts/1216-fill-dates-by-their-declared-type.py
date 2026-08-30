# Fill dates by their declared type
# 29/08 21:41

import io
p="/tmp/claude-0/-home-user-adrianoboller/34595649-0af6-575a-8f79-80dbe8cb7a5d/scratchpad/popular.py"
s=io.open(p,encoding="utf-8").read()
s=s.replace('cols=[c["nome"] for c in r["resultado"]["colunas"] if not c.get("sistema")]',
            'colobj=[c for c in r["resultado"]["colunas"] if not c.get("sistema")]\ntipos={c["nome"]:c["tipo"] for c in colobj}\ncols=[c["nome"] for c in colobj]\nprint("tipos:",tipos)')
s=s.replace('elif c in ("data_nascimento","data_cadastro"): l[c]="1985-04-12 08:30:00"',
            'elif tipos[c].startswith("DateTime"): l[c]="1985-04-12 08:30:00"\n            elif tipos[c].startswith("Date"): l[c]="1985-04-12"\n            elif tipos[c].startswith("Time"): l[c]="08:30:00"')
io.open(p,"w",encoding="utf-8").write(s)
