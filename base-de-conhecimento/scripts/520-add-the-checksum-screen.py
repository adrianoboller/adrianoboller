# Add the checksum screen
# 28/08 16:45

p='crates/phxsql-server/ui/index.html'
s=open(p).read()
a='''async function reindexarTabela() {'''
b='''/// A impressão digital da tabela, para comparar duas cópias.
///
/// Depende da ORDEM de propósito: aqui a ordem de digitação é o dado, e duas
/// tabelas com as mesmas linhas em ordem diferente não são a mesma tabela.
async function checksumTabela(db, tab) {
  db = db || (est.atual || {}).db;
  tab = tab || (est.atual || {}).tab;
  if (!db || !tab) return avisar("escolha uma tabela primeiro", true);
  folha(`Soma de verificação de ${tab}`, "lendo…", `<div class="centro">lendo…</div>`);
  let r;
  try { r = await api("checksum", { database: db, tabela: tab }); }
  catch (e) { return folha(`Soma de verificação de ${tab}`, "", `<div class="aviso mal">${esc(String(e))}</div>`); }
  folha(`Soma de verificação de ${esc(db)}.${esc(tab)}`,
    "para comparar duas cópias sem transportar as duas",
    `<div class="fichas">
       ${ficha(`<code>${esc(r.checksum)}</code>`, "soma", "64 bits, em hexadecimal")}
       ${ficha(fmt(r.linhas), "linhas vivas")}
       ${ficha(fmt(r.slots), "slots", "inclui os excluídos")}
       ${ficha(fmt(r.ms), "ms")}
     </div>
     <div class="aviso"><b>Serve para responder «estas duas tabelas são a
       mesma?»</b> sem transportar as duas — conferir uma réplica contra a
       origem, ou provar que um backup restaurado ficou igual. O
       <code>conferir-backup</code> compara <em>arquivo</em>, que é mais forte
       do que preciso: dois <code>.reg</code> podem diferir no enchimento e ter
       o mesmo dado.</div>
     <div class="aviso"><b>A conta depende da ordem.</b> É de propósito: aqui a
       ordem de digitação <em>é</em> o dado, e duas tabelas com as mesmas linhas
       em ordem diferente não são a mesma tabela. Slot excluído não entra —
       se entrasse, restaurar um backup daria outro número só porque os buracos
       caem em outro lugar.</div>
     <div class="acoes">
       <button class="botao secundario" id="btVoltaCk">← Gerir tabelas</button>
     </div>`);
  $("#btVoltaCk").onclick = () => gerirTabelasAtual();
}

async function reindexarTabela() {'''
assert a in s; s=s.replace(a,b,1)
a='''    { rot:"Verificar",            ico:"✓", quando:comTabela, faz:verificarTabela },'''
b='''    { rot:"Verificar",            ico:"✓", quando:comTabela, faz:verificarTabela },
    { rot:"Soma de verificação",  ico:"⑈", quando:comTabela, faz:() => checksumTabela() },'''
assert a in s; s=s.replace(a,b,1)
open(p,'w').write(s)
print('ok')
