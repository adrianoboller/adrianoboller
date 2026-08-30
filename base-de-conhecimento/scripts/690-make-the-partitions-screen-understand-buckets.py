# Make the partitions screen understand buckets
# 28/08 19:01

import io
p='crates/phxsql-server/ui/index.html'
s=io.open(p,encoding='utf-8').read()

velho='''function volumesDe(e) {
  const pag = e.paginacao;
  if (!pag || !pag.registros_por_arquivo) return [];
  const larg = pag.digitos || 3;
  const total = Math.max(Number(e.slots) || 0, 0);
  const nome = v => `${e.tabela}_${String(v).padStart(larg, "0")}.reg`;
'''
novo='''function volumesDe(e) {
  const pag = e.paginacao;
  if (!pag || !pag.registros_por_arquivo) return [];
  const larg = pag.digitos || 3;
  const total = Math.max(Number(e.slots) || 0, 0);
  const nome = v => `${e.tabela}_${String(v).padStart(larg, "0")}.reg`;

  // Na alfanumérica os baldes vêm PRONTOS do servidor, com quantas linhas cada
  // um tem. Calcular aqui daria errado: `slots` nesta partição é a marca
  // d'água — o maior rowid que já existiu —, e não uma contagem. Dividir a
  // marca d'água pelo teto por letra diria que o balde A tem 36 mil linhas.
  if (pag.baldes && pag.baldes.length) {
    return pag.baldes.map(b => ({
      volume: b.volume, arquivo: b.arquivo, letra: b.letra, existe: b.existe,
      de: b.primeiro_rowid,
      ate: b.primeiro_rowid + pag.registros_por_arquivo - 1,
      usados: b.registros,
    }));
  }
'''
assert velho in s
s=s.replace(velho,novo,1)

velho2='''  const vols = volumesDe(e);
  const porPeriodo = pag.modo && pag.modo !== "quantidade";
  const gb = n => (n / (1024 * 1024 * 1024)).toFixed(2);
  folha(`Partições de ${tab}`,
    `${db} · ${vols.length} volume(s) do .reg · ${
      porPeriodo ? `corta ${pag.modo}, pela coluna ${pag.coluna}` : "corta por quantidade"}`,
    `<div class="fichas">
       <div class="ficha"><div class="v">${esc(porPeriodo ? pag.modo : "por faixa")}</div>
         <div class="r">regra de corte</div>
         <div class="u">${porPeriodo ? esc(`pela coluna ${pag.coluna}`) : "por quantidade"}</div></div>'''
novo2='''  const vols = volumesDe(e);
  const porLetra = pag.modo === "letra";
  const porPeriodo = pag.modo && pag.modo !== "quantidade" && !porLetra;
  const gb = n => (n / (1024 * 1024 * 1024)).toFixed(2);
  const comLinhas = vols.filter(v => v.usados > 0).length;
  folha(`Partições de ${tab}`,
    `${db} · ${porLetra
        ? `${comLinhas} de ${vols.length} baldes com linha · pela primeira letra de ${pag.coluna}`
        : `${vols.length} volume(s) do .reg · ${
            porPeriodo ? `corta ${pag.modo}, pela coluna ${pag.coluna}` : "corta por quantidade"}`}`,
    `<div class="fichas">
       <div class="ficha"><div class="v">${esc(porLetra ? "alfanumérica" : porPeriodo ? pag.modo : "por faixa")}</div>
         <div class="r">regra de corte</div>
         <div class="u">${porLetra || porPeriodo ? esc(`pela coluna ${pag.coluna}`) : "por quantidade"}</div></div>'''
assert velho2 in s
s=s.replace(velho2,novo2,1)

velho3='''       <div class="ficha"><div class="v">${esc(String(pag.registros_por_arquivo))}</div>
         <div class="r">${porPeriodo ? "teto por volume" : "registros por arquivo"}</div></div>'''
novo3='''       <div class="ficha"><div class="v">${esc(String(pag.registros_por_arquivo))}</div>
         <div class="r">${porLetra ? "teto POR LETRA" : porPeriodo ? "teto por volume" : "registros por arquivo"}</div></div>'''
assert velho3 in s
s=s.replace(velho3,novo3,1)

velho4='''    tabela([{t:"volume",cls:"num"},{t:"arquivo"},
            ...(porPeriodo ? [{t:"período que o abriu"}] : []),
            {t:"do rowid",cls:"num"},{t:"até o rowid",cls:"num"},{t:"slots usados",cls:"num"}],
      vols, v => `<tr>
        <td class="num dado">${v.volume}</td>
        <td class="dado"><code>${esc(v.arquivo)}</code></td>
        ${porPeriodo ? `<td class="dado">${esc(v.periodo || "—")}</td>` : ""}
        <td class="num">${v.de}</td><td class="num">${v.ate}</td>
        <td class="num">${v.usados}</td></tr>`) +'''
novo4='''    tabela([{t:"volume",cls:"num"},{t:"arquivo"},
            ...(porPeriodo ? [{t:"período que o abriu"}] : []),
            ...(porLetra ? [{t:"existe"}] : []),
            {t:"do rowid",cls:"num"},{t:"até o rowid",cls:"num"},{t:"slots usados",cls:"num"}],
      vols, v => `<tr${porLetra && !v.usados ? ' class="balde-vazio"' : ""}>
        <td class="num dado">${v.volume}${porLetra ? ` <b>${esc(v.letra)}</b>` : ""}</td>
        <td class="dado"><code>${esc(v.arquivo)}</code></td>
        ${porPeriodo ? `<td class="dado">${esc(v.periodo || "—")}</td>` : ""}
        ${porLetra ? `<td>${v.existe ? "sim" : "<span class=\\"leg\\">ainda não</span>"}</td>` : ""}
        <td class="num">${v.de}</td><td class="num">${v.ate}</td>
        <td class="num">${v.usados}</td></tr>`) +'''
assert velho4 in s
s=s.replace(velho4,novo4,1)

velho5='''    `<div class="nota">
       ${porPeriodo ? `'''
novo5='''    `<div class="nota">
       ${porLetra ? `
       <p><strong>Um arquivo por letra inicial de
       <code>${esc(pag.coluna)}</code>.</strong> A linha vai para o arquivo
       dela, e o endereço continua saindo de uma conta:
       <code>rowid = (balde − 1) × ${esc(String(pag.registros_por_arquivo))} + slot</code>.
       O balde que nunca recebeu linha não ganha arquivo — os 37 estão previstos,
       nem todos existem.</p>
       <p><strong>O teto é por letra, e não da tabela.</strong> Num cadastro
       brasileiro o <code>_S</code> enche muito antes do <code>_K</code>: quem
       enche primeiro derruba a inserção daquela letra, com as outras 36 ainda
       com espaço. É a conta a fazer ao dimensionar.</p>
       <p><strong>A ordem de digitação está no <code>rownum</code>.</strong>
       Aqui o rowid diz em que <em>arquivo</em> a linha está, e não quando ela
       chegou — a leitura sai em ordem alfabética de balde. A ordem de chegada
       é a coluna de sistema, e é por ela que a grade pagina.</p>
       ` : ""}
       ${porPeriodo ? `'''
assert velho5 in s
s=s.replace(velho5,novo5,1)

s=s.replace('''td.ordem{color:var(--texto-3);font-size:11px}''',
            '''td.ordem{color:var(--texto-3);font-size:11px}
tr.balde-vazio{opacity:.45}''',1)
io.open(p,'w',encoding='utf-8').write(s)
