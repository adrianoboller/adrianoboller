# Make irAba redraw tabs when missing
# 28/08 10:36

import pathlib
p = pathlib.Path('crates/phxsql-server/ui/index.html')
s = p.read_text()

v = '''function irAba(qual) {
  if (!est.atual) return;
  est.aba = qual;
  desenharAba();
}'''
n = '''/** Vai para uma das cinco abas da tabela escolhida.
 *
 * Se a barra de abas nao estiver na tela -- porque quem chegou aqui veio de
 * uma folha, como a gestao de tabelas --, reabre a tabela em vez de so
 * desenhar o miolo. Sem isso a Estrutura aparecia solta, sem as abas ao lado
 * para voltar para o Conteudo. */
function irAba(qual) {
  if (!est.atual) return;
  est.aba = qual;
  const { db, tab } = est.atual;
  if (!$("#abas").children.length) return abrirTabela(db, tab);
  return desenharAba();
}'''
assert s.count(v) == 1
s = s.replace(v, n)

v = '''     () => { abrirTabela(db, tab); est.aba = "estrutura"; desenharAba(); }],'''
n = '''     () => { est.aba = "estrutura"; return abrirTabela(db, tab); }],'''
assert s.count(v) == 1
s = s.replace(v, n)
p.write_text(s)
print('ok')
