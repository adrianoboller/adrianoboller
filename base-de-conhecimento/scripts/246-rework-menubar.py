# Rework menubar
# 28/08 10:35

import pathlib
p = pathlib.Path('crates/phxsql-server/ui/index.html')
s = p.read_text()

v = '''  ["Tabela", "T", [
    { rot:"Estrutura",   ico:"▤", tecla:"Alt+2", quando:comTabela, faz:() => irAba("estrutura") },
    { rot:"Conteúdo",    ico:"▦", tecla:"Alt+3", quando:comTabela, faz:() => irAba("conteudo") },
    { rot:"Índices",     ico:"⑂",               quando:comTabela, faz:() => irAba("indices") },
    { rot:"Diário",      ico:"◷",               quando:comTabela, faz:() => irAba("diario") },
    { rot:"Integridade", ico:"⚑",               quando:comTabela, faz:() => irAba("integridade") },
    "sep",
    { rot:"Verificar",              ico:"✓", quando:comTabela, faz:verificarTabela },
    { rot:"Reindexar…",             ico:"↻", quando:comTabela, faz:reindexarTabela },
    { rot:"Reparar pelo espelho…",  ico:"⛨", quando:comTabela, faz:repararTabela },
  ]],
'''

n = '''  // Um menu so para tabela, e nao dois: "Tabela" (a escolhida na arvore) e
  // "Tabelas" (as do database) seriam vizinhos parecidos demais, e quem le a
  // barra teria de adivinhar em qual dos dois esta a operacao que quer.
  ["Tabelas", "T", [
    { rot:"Gerir as tabelas deste banco", ico:"▦", tecla:"Alt+5", faz:gerirTabelasAtual },
    { rot:"Nova tabela…",                 ico:"✚", faz:() => telaNovaTabela(databaseCorrente()) },
    "sep",
    { rot:"Estrutura da tabela",  ico:"▤", tecla:"Alt+2", quando:comTabela, faz:() => irAba("estrutura") },
    { rot:"Editar conteúdo",      ico:"▦", tecla:"Alt+3", quando:comTabela,
      faz:() => verConteudoEditavel(est.atual.db, est.atual.tab) },
    { rot:"Partições da tabela",  ico:"◫", quando:comTabela,
      faz:() => verParticoes(est.atual.db, est.atual.tab) },
    { rot:"Índices",              ico:"⑂", quando:comTabela, faz:() => irAba("indices") },
    { rot:"Diário",               ico:"◷", quando:comTabela, faz:() => irAba("diario") },
    { rot:"Integridade",          ico:"⚑", quando:comTabela, faz:() => irAba("integridade") },
    "sep",
    { rot:"Duplicar tabela…",     ico:"⧉", quando:comTabela,
      faz:() => duplicarTabelaPara(est.atual.db, est.atual.tab) },
    { rot:"Verificar",            ico:"✓", quando:comTabela, faz:verificarTabela },
    { rot:"Reparar índice…",      ico:"↻", quando:comTabela, faz:reindexarTabela },
    { rot:"Reparar tabela pelo espelho…", ico:"⛨", quando:comTabela, faz:repararTabela },
    "sep",
    { rot:"Excluir tabela…",      ico:"🗑", quando:comTabela,
      faz:() => excluirTabelaDe(est.atual.db, est.atual.tab) },
  ]],
'''
assert s.count(v) == 1
s = s.replace(v, n)

# ---------------------------------------------- o menu Ferramentas, espelho da barra
v = '''  ["Ver", "V", ['''
n = '''  // A barra de ferramentas so se alcanca com o mouse. Este menu e a mesma
  // lista pelo teclado -- e onde mora a "Gestão de transações", que nao e
  // ferramenta de tabela nem de administracao.
  ["Ferramentas", "F", [
    { rot:"Gestão de transações", ico:"⇄", faz:verTransacoes },
    { rot:"Consulta SQL",         ico:"⌕", faz:abrirConsulta },
    "sep",
    { rot:"Serviço",              ico:"⏻", faz:verServico },
    { rot:"Conexões",             ico:"⇋", faz:verConexoes },
    { rot:"Replicação",           ico:"⇉", faz:verReplicacao },
    { rot:"Reparar…",             ico:"⛨", faz:repararPeloMenu },
  ]],

  ["Ver", "V", ['''
assert s.count(v) == 1
s = s.replace(v, n)
p.write_text(s)
print('menu Tabelas e menu Ferramentas')
