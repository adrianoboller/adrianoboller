# Honest attachment display plus single-row view
# 28/08 17:55

import io
p='crates/phxsql-server/ui/index.html'
s=io.open(p,encoding='utf-8').read()

velho='''  const cols = (d.colunas || []).filter(c => !c.sistema);
  const itens = d.descartadas || [];'''
novo='''  const cols = (d.colunas || []).filter(c => !c.sistema);
  const itens = d.descartadas || [];
  const EXTERNO = t => /^(Bin|Memo)$/.test(String(t || ""));
  // Um campo externo vazio na listagem quer dizer «não carreguei», e não
  // «não tinha». Dizer NULL nos dois casos faria quem investiga concluir que
  // a foto nunca existiu — que é exatamente o oposto do que a lixeira serve
  // para provar. O botão «ver» busca aquela linha inteira.
  const celulaDa = (item, c) =>
    (!d.anexos_carregados && EXTERNO(c.tipo) && item.anexos > 0)
      ? `<td class="dado nulo">anexo · não carregado</td>`
      : celulaValor((item.linha || {})[c.nome]);'''
assert velho in s
s=s.replace(velho,novo,1)

velho2='''           [{ t: "quando" }, { t: "rowid" }, { t: "quem" }, { t: "anexos" },
            ...cols.map(c => ({ t: c.rotulo || c.nome }))],
           itens,
           d_ => `<tr>
             <td class="dado">${esc(d_.quando)}</td>
             <td class="num">${d_.rowid}</td>
             <td>${esc(d_.usuario_nome || (d_.usuario ? "#" + d_.usuario : "—"))}</td>
             <td class="num">${d_.anexos}</td>
             ${d_.aviso
               ? `<td colspan="${cols.length}" class="mal">${esc(d_.aviso)}</td>`
               : cols.map(c => celulaValor((d_.linha || {})[c.nome])).join("")}
           </tr>`)}'''
novo2='''           [{ t: "quando" }, { t: "rowid" }, { t: "quem" }, { t: "anexos", cls: "num" },
            ...cols.map(c => ({ t: c.rotulo || c.nome })), { t: "" }],
           itens,
           d_ => `<tr>
             <td class="dado">${esc(d_.quando)}</td>
             <td class="num">${d_.rowid}</td>
             <td>${esc(d_.usuario_nome || (d_.usuario ? "#" + d_.usuario : "—"))}</td>
             <td class="num">${d_.anexos}</td>
             ${d_.aviso
               ? `<td colspan="${cols.length}" class="mal">${esc(d_.aviso)}</td>`
               : cols.map(c => celulaDa(d_, c)).join("")}
             <td>${d_.anexos > 0 && !d.anexos_carregados
               ? `<button class="botao mini ver-lixo" data-uuid="${esc(d_.uuid)}">ver inteira</button>`
               : ""}</td>
           </tr>`)}'''
assert velho2 in s
s=s.replace(velho2,novo2,1)

velho3='''  $("#btVoltaLix").onclick = () => gerirTabelasAtual();
  $("#btVerMotivos").onclick = () => telaMotivos(db, tab);'''
novo3='''  $("#btVoltaLix").onclick = () => gerirTabelasAtual();
  $("#btVerMotivos").onclick = () => telaMotivos(db, tab);
  $$("#painel .ver-lixo").forEach(b => b.onclick = () => verLinhaDescartada(db, tab, b.dataset.uuid));'''
assert velho3 in s
s=s.replace(velho3,novo3,1)

# a tela de uma linha so
velho4='''/* ------------------------------------------------------------ os motivos */'''
novo4='''/** Uma linha da lixeira, inteira, com os anexos. */
async function verLinhaDescartada(db, tab, uuid) {
  let d;
  try {
    d = await api("lixeira", { database: db, tabela: tab, uuid });
  } catch (e) { return avisar(String(e), true); }
  const item = (d.descartadas || [])[0];
  if (!item) return avisar("essa linha não está mais na lixeira", true);
  const cols = (d.colunas || []).filter(c => !c.sistema);

  folha(`Linha descartada`, `${esc(db)} · ${esc(tab)} · rowid ${item.rowid}`,
    `<div class="fichas">
       <div class="ficha"><div class="v">${esc(item.quando)}</div><div class="r">quando</div></div>
       <div class="ficha"><div class="v">${esc(item.usuario_nome || "—")}</div><div class="r">quem excluiu</div></div>
       <div class="ficha"><div class="v">${item.anexos}</div><div class="r">anexos</div></div>
       <div class="ficha"><div class="v">${fmtBytes(item.bytes)}</div><div class="r">no .trash</div></div>
     </div>
     ${item.aviso ? `<div class="aviso mal">${esc(item.aviso)}</div>` : ""}
     ${tabela([{ t: "coluna" }, { t: "valor" }], cols,
        c => `<tr><td class="dado"><b>${esc(c.rotulo || c.nome)}</b>
                <span class="leg">${esc(c.tipo)}</span></td>
              ${celulaValor((item.linha || {})[c.nome])}</tr>`)}
     <p class="leg">O <code>uuid</code> deste descarte é
       <code>${esc(item.uuid)}</code>. É um v7: ordenar por ele é ordenar por
       quando aconteceu.</p>
     <div class="acoes">
       <button class="botao secundario" id="btVoltaLinha">← Lixeira</button>
     </div>`);
  $("#btVoltaLinha").onclick = () => telaLixeira(db, tab);
}

/* ------------------------------------------------------------ os motivos */'''
assert velho4 in s
s=s.replace(velho4,novo4,1)
io.open(p,'w',encoding='utf-8').write(s)
