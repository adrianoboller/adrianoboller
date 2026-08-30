# Fix the checkbox layout and run all checks
# 28/08 23:03

import pathlib
p = pathlib.Path("crates/phxsql-server/ui/index.html")
s = p.read_text()
antigo = """.mini-campo.caixa{flex-direction:row;align-items:center;gap:6px;text-transform:none;
  letter-spacing:0;font-size:12px;padding-bottom:8px}
.mini-campo.caixa input{min-width:0}"""
novo = """/* A caixa de marcar nao herda a largura minima dos campos de texto: sem isto
   ela estica e fica um retangulo vazio do lado do quadradinho. */
.mini-campo.caixa{flex-direction:row;align-items:center;gap:7px;text-transform:none;
  letter-spacing:0;font-size:12.5px;color:var(--texto-2);padding-bottom:7px;
  border:none;background:none;white-space:nowrap}
.mini-campo.caixa input{min-width:0;width:15px;height:15px;padding:0;margin:0;
  accent-color:var(--acao-consultar)}"""
assert antigo in s
s = s.replace(antigo, novo)
p.write_text(s)
