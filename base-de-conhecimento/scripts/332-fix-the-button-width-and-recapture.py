# Fix the button width and recapture
# 28/08 11:44

import pathlib
p = pathlib.Path('crates/phxsql-server/ui/index.html')
s = p.read_text()
v = '''.acoes{
  display:flex;align-items:center;gap:10px;flex-wrap:wrap;margin:14px 0;
}'''
n = '''.acoes{
  display:flex;align-items:center;gap:10px;flex-wrap:wrap;margin:14px 0;
}
/* O `.botao` nasceu com `width:100%` para o cartao de entrada, onde ele e o
   unico da linha. Numa barra de acoes isso o faz ocupar a largura inteira e
   empurrar os outros para baixo -- aqui ele cabe no proprio texto. */
.acoes .botao{width:auto;padding:8px 16px}'''
assert s.count(v) == 1
p.write_text(s.replace(v, n))
print('ok')
