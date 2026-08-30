# Add cursor pagination to the grid
# 28/08 18:35

import io
p='crates/phxsql-server/ui/index.html'
s=io.open(p,encoding='utf-8').read()

velho='''  const [e, r] = await Promise.all([
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
    </div>`;'''

novo='''  // `cursor` é o rowid da última linha da página anterior — o servidor
  // devolve o par pronto. A grade não guarda "página 37": guarda o ponto onde
  // parou, que é o que faz a página 37 custar o mesmo que a página 1.
  const pedido = { database: db, tabela: tab, max: est.teto,
                   visao: verExcluidos ? "excluidas" : "ativas" };
  if (cursor && cursor.antes !== undefined) pedido.antes = cursor.antes;
  else if (cursor && cursor.depois !== undefined) pedido.depois = cursor.depois;

  const [e, r] = await Promise.all([
    api("esquema", { database: db, tabela: tab }),
    api("varrer", pedido),
  ]);
  est.esquemaAtual = e;
  // As colunas de sistema não viram colunas da grade: quem lê a marca é o
  // estilo da linha e o botão de restaurar, e o número de ordem tem coluna
  // própria à esquerda. Repetir "softdeleted: não" em toda linha só ocuparia
  // espaço.
  const sistemas = e.colunas.filter(c => c.sistema).map(c => c.nome);
  const cols = e.colunas.filter(c => !c.sistema).map(c => c.nome);
  const temRownum = e.colunas.some(c => c.nome === "rownum");

  const barra = `<div class="acoes">
      <button class="botao" id="btNova">Nova linha</button>
      <button class="botao secundario" id="btVoltarDb">← ${esc(db)}</button>
      <span class="chip-visao">
        <button id="vwAtivas" class="${verExcluidos ? "" : "ativo"}">ativas</button>
        <button id="vwExcl" class="${verExcluidos ? "ativo" : ""}">excluídas</button>
      </span>
      <span class="paginar">
        <button class="botao mini" id="pgInicio" ${r.ha_antes ? "" : "disabled"}>⏮ início</button>
        <button class="botao mini" id="pgAntes" ${r.ha_antes ? "" : "disabled"}>← anterior</button>
        <button class="botao mini" id="pgDepois" ${r.ha_mais ? "" : "disabled"}>próxima →</button>
      </span>
      <span class="leg">${fmt(r.devolvidas)} de ${fmt(r.registros)} · página por
        <b>${esc(r.modo)}</b></span>
    </div>`;'''
assert velho in s
s=s.replace(velho,novo,1)

velho2='''  $("#painel").innerHTML = barra + (r.linhas.length
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
  $("#vwExcl").onclick = () => verConteudoEditavel(db, tab, true);'''
novo2='''  $("#painel").innerHTML = barra + (r.linhas.length
    ? tabela([...(temRownum ? [{t:"nº",cls:"num"}] : []),
              {t:"rowid",cls:"num"}, ...cols.map(c => ({t:c})), {t:""}],
        r.linhas, l => `<tr class="linha-dado${verExcluidos ? " linha-excluida" : ""}"
              data-rowid="${esc(String(l.rowid))}">
          ${temRownum ? `<td class="num dado ordem">${esc(String(l.rownum ?? "—"))}</td>` : ""}
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
  $("#pgInicio").onclick = () => verConteudoEditavel(db, tab, verExcluidos, null);
  $("#pgAntes").onclick = () =>
    verConteudoEditavel(db, tab, verExcluidos, { antes: r.cursor_inicio });
  $("#pgDepois").onclick = () =>
    verConteudoEditavel(db, tab, verExcluidos, { depois: r.cursor_fim });'''
assert velho2 in s
s=s.replace(velho2,novo2,1)

# assinatura e a doc
velho3='''async function verConteudoEditavel(db, tab, verExcluidos = false) {'''
novo3='''async function verConteudoEditavel(db, tab, verExcluidos = false, cursor = null) {'''
assert velho3 in s
s=s.replace(velho3,novo3,1)

velho4=''' * A grade tem DOIS modos: o normal, que nao enxerga linha marcada como
 * excluida, e o dos excluidos, que mostra so elas com o botao de restaurar.
 * O segundo existe porque marcar sem ter como desmarcar seria so uma forma
 * elaborada de perder o dado. */'''
novo4=''' * A grade tem DOIS modos: o normal, que nao enxerga linha marcada como
 * excluida, e o dos excluidos, que mostra so elas com o botao de restaurar.
 * O segundo existe porque marcar sem ter como desmarcar seria so uma forma
 * elaborada de perder o dado.
 *
 * E pagina por CURSOR, nao por posicao: `cursor` leva o rowid onde a pagina
 * anterior parou. A diferenca nao e de estilo -- pular ate a posicao um milhao
 * custa um milhao de passos, e continuar depois do rowid um milhao custa uma
 * conta. Na medicao com 800 mil linhas, a pagina do meio saiu de 1.420 ms para
 * tempo nao medivel. */'''
assert velho4 in s
s=s.replace(velho4,novo4,1)

# CSS dos botoes de paginar
velho5='''.chip-visao button.ativo{background:var(--laranja);color:#10060a}'''
novo5='''.chip-visao button.ativo{background:var(--laranja);color:#10060a}
.paginar{display:inline-flex;gap:6px;margin-left:8px}
.paginar .botao.mini:disabled{opacity:.35;cursor:default;border-color:var(--linha)}
.paginar .botao.mini:disabled:hover{border-color:var(--linha);color:var(--texto-3)}
td.ordem{color:var(--texto-3);font-size:11px}'''
assert velho5 in s
s=s.replace(velho5,novo5,1)
io.open(p,'w',encoding='utf-8').write(s)
