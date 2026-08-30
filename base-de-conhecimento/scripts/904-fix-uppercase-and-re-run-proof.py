# Fix uppercase and re-run proof
# 29/08 00:00

import pathlib
p = pathlib.Path("crates/phxsql-server/ui/index.html")
s = p.read_text()
s = s.replace(
'''table.conf .esc{display:flex;gap:7px;align-items:baseline;cursor:pointer;margin:0}''',
'''/* O `label` da folha nasce em caixa alta -- e ali ele e ROTULO. Aqui dentro
   ele embrulha um VALOR, e valor mostrado em caixa alta e uma mentira: quem
   olha nao sabe se «BLUMENAU» esta gravado assim ou se e a tela gritando. */
table.conf .esc{display:flex;gap:7px;align-items:baseline;cursor:pointer;margin:0;
  text-transform:none;letter-spacing:0;font-size:12px;color:var(--texto)}''', 1)
p.write_text(s)
print("ok")
