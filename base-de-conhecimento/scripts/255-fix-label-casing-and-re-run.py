# Fix label casing and re-run
# 28/08 10:45

import pathlib
p = pathlib.Path('crates/phxsql-server/ui/index.html')
s = p.read_text()
v = '''.criar label{display:flex;flex-direction:column;gap:5px;font-size:11.5px}
.criar label.largo{grid-column:1/-1}
.criar label > span{color:var(--texto-3)}
.criar label em{font-style:normal;opacity:.7}'''
n = '''.criar label{display:flex;flex-direction:column;gap:5px;font-size:11.5px}
.criar label.largo{grid-column:1/-1}
/* Caixa-alta e a regra da casa para rotulo curto, e estraga rotulo com
   parentese: "TETO DE VOLUMES (0 = O QUE COUBER NO SUFIXO)" grita e quebra em
   duas linhas. Aqui o rotulo fica em caixa normal e a dica desce sozinha. */
.criar label > span{
  color:var(--texto-3);text-transform:none;letter-spacing:.02em;
  display:flex;flex-direction:column;gap:2px;
}
.criar label em{font-style:normal;opacity:.72;font-size:10.5px}
.criar label em code{font-size:10px}'''
assert s.count(v) == 1
p.write_text(s.replace(v, n))
print('ok')
