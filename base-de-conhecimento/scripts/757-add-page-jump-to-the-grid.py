# Add page jump to the grid
# 28/08 19:50

import pathlib
p = pathlib.Path("crates/phxsql-server/ui/index.html")
s = p.read_text()

antigo = """ * E pagina por CURSOR, nao por posicao: `cursor` leva o rowid onde a pagina
 * anterior parou. A diferenca nao e de estilo -- pular ate a posicao um milhao
 * custa um milhao de passos, e continuar depois do rowid um milhao custa uma
 * conta. Na medicao com 800 mil linhas, a pagina do meio saiu de 1.420 ms para
 * tempo nao medivel. */
async function verConteudoEditavel(db, tab, verExcluidos = false, cursor = null) {"""
novo = """ * Anda por CURSOR e SALTA por posicao, que sao coisas diferentes e as duas
 * baratas:
 *
 * - anterior/proxima mandam `antes`/`depois` com o rowid onde a pagina parou.
 *   Continuar depois do rowid um milhao e uma conta, nao uma procura.
 * - «ir para a pagina N» manda `pular`, e o servidor bisseta pelo `rownum` --
 *   164 us contra 246 ms de andar ate la, medido com 800 mil linhas.
 *
 * O numero da pagina e o total so aparecem porque agora sao baratos: o
 * servidor devolve `visiveis` a partir de dois contadores do cabecalho, sem
 * varrer. Era por nao existir esse numero que a grade nao dizia «de quantas». */
async function verConteudoEditavel(db, tab, verExcluidos = false, cursor = null) {"""
assert antigo in s
s = s.replace(antigo, novo)

antigo = """  const pedido = { database: db, tabela: tab, max: est.teto,
                   visao: verExcluidos ? "excluidas" : "ativas" };
  if (cursor && cursor.antes !== undefined) pedido.antes = cursor.antes;
  else if (cursor && cursor.depois !== undefined) pedido.depois = cursor.depois;
"""
novo = """  const pedido = { database: db, tabela: tab, max: est.teto,
                   visao: verExcluidos ? "excluidas" : "ativas" };
  if (cursor && cursor.antes !== undefined) pedido.antes = cursor.antes;
  else if (cursor && cursor.depois !== undefined) pedido.depois = cursor.depois;
  else if (cursor && cursor.pular) pedido.pular = cursor.pular;
"""
assert antigo in s
s = s.replace(antigo, novo)

antigo = """      <span class="paginar">
        <button class="botao mini" id="pgInicio" ${r.ha_antes ? "" : "disabled"}>⏮ início</button>
        <button class="botao mini" id="pgAntes" ${r.ha_antes ? "" : "disabled"}>← anterior</button>
        <button class="botao mini" id="pgDepois" ${r.ha_mais ? "" : "disabled"}>próxima →</button>
      </span>
      <span class="leg">${fmt(r.devolvidas)} de ${fmt(r.registros)} · página por
        <b>${esc(r.modo)}</b></span>
    </div>`;
"""
novo = """      <span class="paginar">
        <button class="botao mini" id="pgInicio" ${r.ha_antes ? "" : "disabled"}>⏮ início</button>
        <button class="botao mini" id="pgAntes" ${r.ha_antes ? "" : "disabled"}>← anterior</button>
        <button class="botao mini" id="pgDepois" ${r.ha_mais ? "" : "disabled"}>próxima →</button>
        <button class="botao mini" id="pgFim" ${paginas > 1 ? "" : "disabled"}>fim ⏭</button>
        <input id="pgIr" class="campo-pagina" type="number" min="1" max="${paginas}"
               value="${pagina}" title="ir para a página" ${paginas > 1 ? "" : "disabled"}>
        <span class="leg">de ${fmt(paginas)}</span>
      </span>
      <span class="leg">${fmt(r.devolvidas)} de ${fmt(r.visiveis ?? r.registros)} ·
        <b>${esc(r.modo)}</b>${r.salto ? ` · ${esc(r.salto)}` : ""}</span>
    </div>`;
"""
assert antigo in s
s = s.replace(antigo, novo)

antigo = """  const sistemas = e.colunas.filter(c => c.sistema).map(c => c.nome);
  const cols = e.colunas.filter(c => !c.sistema).map(c => c.nome);
  const temRownum = e.colunas.some(c => c.nome === "rownum");
"""
novo = """  const sistemas = e.colunas.filter(c => c.sistema).map(c => c.nome);
  const cols = e.colunas.filter(c => !c.sistema).map(c => c.nome);
  const temRownum = e.colunas.some(c => c.nome === "rownum");

  // Quantas páginas, e em qual estamos. O total é exato porque `visiveis` é
  // exato; o número da página é CONTADO pela navegação (uma a mais quando
  // avança, uma a menos quando volta) porque o cursor não sabe a posição
  // dele -- e não saber é justamente o que faz ele ser barato.
  const paginas = Math.max(1, Math.ceil((r.visiveis ?? r.registros) / est.teto));
  const pagina = Math.min(paginas, Math.max(1, (cursor && cursor.pagina) || 1));
"""
assert antigo in s
s = s.replace(antigo, novo)

antigo = """  $("#pgInicio").onclick = () => verConteudoEditavel(db, tab, verExcluidos, null);
  $("#pgAntes").onclick = () =>
    verConteudoEditavel(db, tab, verExcluidos, { antes: r.cursor_inicio });
  $("#pgDepois").onclick = () =>
    verConteudoEditavel(db, tab, verExcluidos, { depois: r.cursor_fim });"""
novo = """  $("#pgInicio").onclick = () => verConteudoEditavel(db, tab, verExcluidos, null);
  $("#pgAntes").onclick = () =>
    verConteudoEditavel(db, tab, verExcluidos,
                        { antes: r.cursor_inicio, pagina: pagina - 1 });
  $("#pgDepois").onclick = () =>
    verConteudoEditavel(db, tab, verExcluidos,
                        { depois: r.cursor_fim, pagina: pagina + 1 });
  // O salto: manda a POSIÇÃO, e o servidor bisseta pelo rownum quando pode.
  const irPara = n => {
    const alvo = Math.min(paginas, Math.max(1, Math.round(n) || 1));
    if (alvo === 1) return verConteudoEditavel(db, tab, verExcluidos, null);
    verConteudoEditavel(db, tab, verExcluidos,
                        { pular: (alvo - 1) * est.teto, pagina: alvo });
  };
  $("#pgFim").onclick = () => irPara(paginas);
  $("#pgIr").onchange = ev => irPara(+ev.target.value);
  $("#pgIr").onkeydown = ev => { if (ev.key === "Enter") irPara(+ev.target.value); };"""
assert antigo in s
s = s.replace(antigo, novo)
p.write_text(s)
print("ok")
