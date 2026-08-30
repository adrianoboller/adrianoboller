# Colour the action buttons
# 28/08 22:49

import pathlib
p = pathlib.Path("crates/phxsql-server/ui/index.html")
s = p.read_text()

trocas = [
 ('<button class="botao" id="btNova">Nova linha</button>',
  '<button class="botao incluir" id="btNova">Nova linha</button>'),
 ('<button class="botao mini restaurar" data-rowid="${esc(String(l.rowid))}">restaurar</button>',
  '<button class="botao mini incluir restaurar" data-rowid="${esc(String(l.rowid))}">restaurar</button>'),
 ('<button class="botao mini" id="btRestaurarFicha">restaurar</button>',
  '<button class="botao mini incluir" id="btRestaurarFicha">restaurar</button>'),
 ('<button class="botao" id="btSalvar">${novo ? "Incluir" : "Salvar"}</button>',
  '<button class="botao ${novo ? "incluir" : "alterar"}" id="btSalvar">${novo ? "Incluir" : "Salvar"}</button>'),
 ('<button class="botao perigo" id="btExcluir">Excluir</button>',
  '<button class="botao excluir" id="btExcluir">Excluir</button>'),
 ('<button class="botao" id="btImportar" disabled>Gravar</button>',
  '<button class="botao incluir" id="btImportar" disabled>Gravar</button>'),
 ('<button class="botao" id="btConsultar">Consultar</button>',
  '<button class="botao consultar" id="btConsultar">Consultar</button>'),
]
for a, b in trocas:
    assert a in s, a[:60]
    s = s.replace(a, b)
p.write_text(s)
print("ok")
