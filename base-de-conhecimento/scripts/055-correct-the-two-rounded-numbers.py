# Correct the two rounded numbers
# 27/08 19:13

p="/tmp/claude-0/-home-user-adrianoboller/34595649-0af6-575a-8f79-80dbe8cb7a5d/scratchpad/artefato/dossie-phxsql.html"
s=open(p).read()
s=s.replace('<div class="v">12.400</div>','<div class="v">11.775</div>')
s=s.replace('<div class="v">2.000</div>','<div class="v">1.918</div>')
s=s.replace('PhxSql 0.2.0 · 12.400 linhas de Rust','PhxSql 0.2.0 · 11.775 linhas de Rust')
open(p,'w').write(s)
