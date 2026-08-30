# Fix the edit form arity bug
# 28/08 22:17

import pathlib
p = pathlib.Path("crates/phxsql-server/ui/index.html")
s = p.read_text()
antigo = """  const sistema = (e.colunas.find(c => c.sistema) || {}).nome || "softdeleted";
  const editaveis = e.colunas.filter(c => c.nome !== sistema);
  const marcada = linha[sistema] === true;"""
novo = """  // TODAS as colunas de sistema ficam de fora — não só a primeira. Enquanto
  // era `find(c => c.sistema)`, só o `softdeleted` saía e o `rownum` continuava
  // na ficha: o formulário mandava 8 valores para uma tabela de 9 colunas, e
  // TODO salvar e TODO incluir pela tela falhavam com «a lista tem 8 valores».
  // O servidor completa as duas no fim — herdando o valor anterior, que é o
  // que impede um salvar de rotina de ressuscitar linha excluída ou renumerar.
  const editaveis = e.colunas.filter(c => !c.sistema);
  const marcada = linha["softdeleted"] === true;"""
assert antigo in s
s = s.replace(antigo, novo)
p.write_text(s)
print("ok")
