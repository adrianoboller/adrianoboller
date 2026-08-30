# Replace the content table with the grid
# 27/08 21:51

p='crates/phxsql-server/ui/index.html'
s=open(p).read()

velho = s[s.index('async function vConteudo(database, tabela_) {'):s.index('async function vIndices(database, tabela_) {')]
novo = '''/// Mapeia o tipo do PhxSql no tipo que o grid entende.
///
/// So os que mudam a apresentacao: numero alinha a direita, moeda formata,
/// data ordena por instante. O resto e texto, e texto e o padrao do grid.
function tipoDoGrid(t) {
  if (/^Int|^UInt/.test(t)) return "numero";
  if (/^Decimal/.test(t)) return "moeda";
  if (t === "Date" || t === "DateTime") return "dataHora";
  return "texto";
}

async function vConteudo(database, tabela_) {
  const e = await api("esquema", { database, tabela:tabela_ });
  const r = await api("varrer", { database, tabela:tabela_,
                                  indice: est.ordem || undefined, max: est.teto });
  est.linhas = r.linhas;
  est.esquemaAtual = e;
  const opcoes = [`<option value="">ordem de digitação (.reg)</option>`]
    .concat(e.indices.map(i =>
      `<option value="${esc(i.nome)}" ${est.ordem===i.nome?"selected":""}>índice ${esc(i.nome)}</option>`));
  const tetos = [200, 1000, 5000].map(n =>
    `<option value="${n}" ${est.teto===n?"selected":""}>${n} linhas</option>`);
  return `<div class="ferramentas">
      <label for="ord">Percorrer por</label>
      <select id="ord">${opcoes.join("")}</select>
      <label for="teto">Trazer</label>
      <select id="teto">${tetos.join("")}</select>
      <span class="conta">${r.devolvidas} de ${r.total} linhas · ${esc(r.ordem)}</span>
      <span class="dica">Arraste um cabeçalho para a faixa de cima para agrupar.</span>
    </div>
    <div id="grade"></div>`;
}

function ligarConteudo() {
  const s = $("#ord");
  if (s) s.onchange = () => { est.ordem = s.value; desenharAba(); };
  const t = $("#teto");
  if (t) t.onchange = () => { est.teto = Number(t.value); desenharAba(); };

  const alvo = $("#grade");
  if (!alvo || !window.PhxGrid || !est.esquemaAtual) return;

  // O grid do Phoenix, com a faixa de agrupamento ligada. As colunas saem do
  // esquema da tabela -- nada e escrito a mao aqui, entao tabela nova aparece
  // certa sem ninguem mexer nesta pagina.
  const colunas = [{ campo:"rowid", titulo:"rowid", tipo:"numero", largura:90 }]
    .concat(est.esquemaAtual.colunas.map(c => ({
      campo: c.nome,
      titulo: c.nome,
      tipo: tipoDoGrid(c.tipo),
      agregador: /^Int|^UInt|^Decimal/.test(c.tipo) ? "sum" : null
    })));

  if (est.grade && est.grade.destruir) { try { est.grade.destruir(); } catch {} }
  est.grade = PhxGrid.criar("#grade", {
    agrupavel: true,
    buscaGlobal: true,
    colunas: colunas,
    dados: est.linhas,
    pagina: { tamanho: 100, opcoes: [50, 100, 200] }
  });
}

'''
s = s.replace(velho, novo)

# estado novo
s = s.replace('              servidor:"", portaLocal:5000, servidores:[], database:"" };',
              '              servidor:"", portaLocal:5000, servidores:[], database:"",\n'
              '              teto:200, esquemaAtual:null, grade:null };')

# a dica na barra de ferramentas
s = s.replace('.conta{font-family:"IBM Plex Mono",monospace;font-size:12px;color:var(--texto-3)}',
'''.conta{font-family:"IBM Plex Mono",monospace;font-size:12px;color:var(--texto-3)}
.dica{font-size:11.5px;color:var(--texto-3);opacity:.8;font-style:italic;margin-left:auto}''')
open(p,'w').write(s)
print('conteudo trocado pelo grid')
