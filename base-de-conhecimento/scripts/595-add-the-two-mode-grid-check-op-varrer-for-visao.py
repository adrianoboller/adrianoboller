# Add the two-mode grid; check op_varrer for visao
# 28/08 17:49

import io
p='crates/phxsql-server/ui/index.html'
s=io.open(p,encoding='utf-8').read()

velho='''/** A grade de dados, com a coluna de acao que abre a ficha. */
async function verConteudoEditavel(db, tab) {
  est.atual = { db, tab };
  folha(`${tab}`, `${db} · clique numa linha para editar`,
        `<div class="centro">carregando…</div>`);

  const [e, r] = await Promise.all([
    api("esquema", { database: db, tabela: tab }),
    api("varrer", { database: db, tabela: tab, max: est.teto }),
  ]);
  est.esquemaAtual = e;
  const cols = e.colunas.map(c => c.nome);

  const barra = `<div class="acoes">
      <button class="botao" id="btNova">Nova linha</button>
      <button class="botao secundario" id="btVoltarDb">← ${esc(db)}</button>
      <span class="leg">${r.linhas.length} linha(s) · teto de ${est.teto}</span>
    </div>`;

  $("#painel").innerHTML = barra + (r.linhas.length
    ? tabela([{t:"rowid",cls:"num"}, ...cols.map(c => ({t:c})), {t:""}],
        r.linhas, l => `<tr class="linha-dado" data-rowid="${esc(String(l.rowid))}">
          <td class="num dado">${esc(String(l.rowid))}</td>
          ${cols.map(c => celulaValor(l[c])).join("")}
          <td><span class="pino">editar</span></td></tr>`)
    : `<div class="vazio">tabela vazia</div>`);

  $("#btNova").onclick = () => abrirFicha(db, tab, null);
  $("#btVoltarDb").onclick = () => verDatabase(db);
  $$("#painel .linha-dado").forEach(tr =>
    tr.onclick = () => abrirFicha(db, tab, +tr.dataset.rowid));
}'''

novo='''/** A grade de dados, com a coluna de acao que abre a ficha.
 *
 * A grade tem DOIS modos: o normal, que nao enxerga linha marcada como
 * excluida, e o dos excluidos, que mostra so elas com o botao de restaurar.
 * O segundo existe porque marcar sem ter como desmarcar seria so uma forma
 * elaborada de perder o dado. */
async function verConteudoEditavel(db, tab, verExcluidos = false) {
  est.atual = { db, tab };
  folha(`${tab}`, `${db} · clique numa linha para editar`,
        `<div class="centro">carregando…</div>`);

  const [e, r] = await Promise.all([
    api("esquema", { database: db, tabela: tab }),
    api("varrer", { database: db, tabela: tab, max: est.teto,
                    visao: verExcluidos ? "excluidas" : "ativas" }),
  ]);
  est.esquemaAtual = e;
  // A coluna de sistema nao vira coluna da grade: quem a le e o estilo da
  // linha e o botao de restaurar, e um campo "softdeleted: nao" repetido em
  // toda linha so ocuparia espaco.
  const sistema = (e.colunas.find(c => c.sistema) || {}).nome || "softdeleted";
  const cols = e.colunas.filter(c => c.nome !== sistema).map(c => c.nome);

  const barra = `<div class="acoes">
      <button class="botao" id="btNova">Nova linha</button>
      <button class="botao secundario" id="btVoltarDb">← ${esc(db)}</button>
      <span class="chip-visao">
        <button id="vwAtivas" class="${verExcluidos ? "" : "ativo"}">ativas</button>
        <button id="vwExcl" class="${verExcluidos ? "ativo" : ""}">excluídas</button>
      </span>
      <span class="leg">${r.linhas.length} linha(s) · teto de ${est.teto}</span>
    </div>`;

  const vazio = verExcluidos
    ? `<div class="vazio">nenhuma linha marcada como excluída</div>`
    : `<div class="vazio">tabela vazia</div>`;

  $("#painel").innerHTML = barra + (r.linhas.length
    ? tabela([{t:"rowid",cls:"num"}, ...cols.map(c => ({t:c})), {t:""}],
        r.linhas, l => `<tr class="linha-dado${verExcluidos ? " linha-excluida" : ""}"
              data-rowid="${esc(String(l.rowid))}">
          <td class="num dado">${esc(String(l.rowid))}</td>
          ${cols.map(c => celulaValor(l[c])).join("")}
          <td>${verExcluidos
            ? `<button class="botao mini restaurar" data-rowid="${esc(String(l.rowid))}">restaurar</button>`
            : `<span class="pino">editar</span>`}</td></tr>`)
    : vazio);

  $("#btNova").onclick = () => abrirFicha(db, tab, null);
  $("#btVoltarDb").onclick = () => verDatabase(db);
  $("#vwAtivas").onclick = () => verConteudoEditavel(db, tab, false);
  $("#vwExcl").onclick = () => verConteudoEditavel(db, tab, true);
  $$("#painel .linha-dado").forEach(tr =>
    tr.onclick = ev => {
      // O clique no botao de restaurar nao pode abrir a ficha junto.
      if (ev.target.closest(".restaurar")) return;
      abrirFicha(db, tab, +tr.dataset.rowid);
    });
  $$("#painel .restaurar").forEach(b => b.onclick = ev => {
    ev.stopPropagation();
    restaurarLinha(db, tab, +b.dataset.rowid,
      () => verConteudoEditavel(db, tab, true));
  });
}'''
assert velho in s
s=s.replace(velho,novo,1)
io.open(p,'w',encoding='utf-8').write(s)
print('ok')
