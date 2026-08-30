# Route delete through the new dialog
# 28/08 17:44

import io
p='crates/phxsql-server/ui/index.html'
s=io.open(p,encoding='utf-8').read()

velho = '''  if (!novo) $("#btExcluir").onclick = async ev => {
    ev.preventDefault();
    if (!confirm(`Excluir o registro ${rowid} de ${tab}?\\n\\n`
      + `O slot fica marcado como livre e NÃO é reaproveitado — é assim que a `
      + `ordem de digitação se mantém.`)) return;
    try {
      await api("excluir", { database: db, tabela: tab, rowid });
      avisar(`registro ${rowid} excluído`);
      est.esquemaAtual = null;
      verConteudoEditavel(db, tab);
    } catch (err) { avisar(String(err), true); }
  };'''

novo = '''  if (!novo) $("#btExcluir").onclick = ev => {
    ev.preventDefault();
    dialogoExcluir(db, tab, rowid, () => {
      est.esquemaAtual = null;
      verConteudoEditavel(db, tab);
    });
  };'''
assert velho in s
s=s.replace(velho,novo,1)
io.open(p,'w',encoding='utf-8').write(s)
print('ok')
