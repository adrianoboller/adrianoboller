# Fix chart scaling and re-render
# 27/08 22:39

p='crates/phxsql-server/ui/index.html'
s=open(p).read()

# barras(): a largura do desenho passa a ser parametro.
#
# O viewBox de 620 dentro de um cartao de ~370 px encolhia TUDO em 0,6 --
# inclusive o texto, que ia para 7 px. SVG escala o desenho inteiro, entao a
# unica forma de o texto sair no tamanho certo e o viewBox nascer proximo da
# largura real.
s=s.replace('''function barras(itens, opc = {}) {
  if (!itens.length) return `<div class="vazioc">sem dados ainda</div>`;
  const alt = 26, teto = Math.max(...itens.map(i => i.valor), 1);
  const larguraRotulo = opc.rotulo ?? 150;
  const h = itens.length * alt + 8;''',
'''function barras(itens, opc = {}) {
  if (!itens.length) return `<div class="vazioc">sem dados ainda</div>`;
  // Largura próxima da real do cartão: o SVG escala o desenho inteiro, então
  // um viewBox largo demais encolhe o texto junto.
  const L = opc.largura ?? 360;
  const alt = 24, teto = Math.max(...itens.map(i => i.valor), 1);
  const larguraRotulo = opc.rotulo ?? 120;
  const h = itens.length * alt + 6;''')
s=s.replace('''    const w = Math.max(1, (it.valor / teto) * (620 - larguraRotulo - 70));
    const segundo = it.segundo || 0;
    const w2 = segundo ? Math.max(1, (segundo / teto) * (620 - larguraRotulo - 70)) : 0;
    return `<g>
      <text x="0" y="${y + 15}" font-size="12" fill="currentColor" opacity=".85">${esc(it.nome)}</text>
      <rect x="${larguraRotulo}" y="${y + 4}" width="${w}" height="14" rx="3"
            fill="${it.cor || "var(--laranja)"}" opacity=".9"/>
      ${segundo ? `<rect x="${larguraRotulo + w}" y="${y + 4}" width="${w2}" height="14" rx="3"
            fill="var(--log)" opacity=".85"/>` : ""}
      <text x="${larguraRotulo + w + w2 + 8}" y="${y + 15}" font-size="11.5"
            font-family="IBM Plex Mono, monospace" fill="currentColor" opacity=".7">${esc(it.texto ?? fmt(it.valor))}</text>
    </g>`;
  }).join("");
  return `<svg viewBox="0 0 620 ${h}" role="img" aria-label="${esc(opc.titulo || "gráfico de barras")}">${linhas}</svg>`;''',
'''    const util = L - larguraRotulo - (opc.texto ?? 78);
    const w = Math.max(1, (it.valor / teto) * util);
    const segundo = it.segundo || 0;
    const w2 = segundo ? Math.max(1, (segundo / teto) * util) : 0;
    return `<g>
      <text x="0" y="${y + 14}" font-size="11.5" fill="currentColor" opacity=".85">${esc(it.nome)}</text>
      <rect x="${larguraRotulo}" y="${y + 4}" width="${w.toFixed(1)}" height="13" rx="3"
            fill="${it.cor || "var(--laranja)"}" opacity=".9"/>
      ${segundo ? `<rect x="${(larguraRotulo + w).toFixed(1)}" y="${y + 4}" width="${w2.toFixed(1)}"
            height="13" rx="3" fill="var(--log)" opacity=".85"/>` : ""}
      <text x="${(larguraRotulo + w + w2 + 7).toFixed(1)}" y="${y + 14}" font-size="10.5"
            font-family="IBM Plex Mono, monospace" fill="currentColor" opacity=".7">${esc(it.texto ?? fmt(it.valor))}</text>
    </g>`;
  }).join("");
  return `<svg viewBox="0 0 ${L} ${h}" role="img" aria-label="${esc(opc.titulo || "gráfico de barras")}">${linhas}</svg>`;''')

# a area ocupa a carta larga: viewBox proximo de 1.150 px
s=s.replace('''function areaHoras(serie, recusadas) {
  const L = 620, A = 140, base = A - 26, topo = 10;''',
'''function areaHoras(serie, recusadas) {
  // Carta larga: o viewBox nasce largo, senão o texto sairia esticado 2×.
  const L = 1180, A = 210, base = A - 30, topo = 16;''')
s=s.replace('''  const px = i => (i / (serie.length - 1)) * (L - 20) + 10;''',
            '''  const px = i => (i / (serie.length - 1)) * (L - 24) + 12;''')
s=s.replace('''  const area = `10,${base} ${linha} ${(L - 10)},${base}`;''',
            '''  const area = `12,${base} ${linha} ${(L - 12)},${base}`;''')
s=s.replace('''    `<rect x="${(px(i) - 3).toFixed(1)}" y="${py(v).toFixed(1)}" width="6"''',
            '''    `<rect x="${(px(i) - 4).toFixed(1)}" y="${py(v).toFixed(1)}" width="8"''')
s=s.replace('''  const marcas = [0, 6, 12, 18, 23].map(i =>
    `<text x="${px(i).toFixed(1)}" y="${A - 8}" text-anchor="middle" font-size="10"
           fill="currentColor" opacity=".55">${i === 23 ? "agora" : (23 - i) + "h atrás"}</text>`).join("");''',
'''  const marcas = [0, 4, 8, 12, 16, 20, 23].map(i =>
    `<text x="${px(i).toFixed(1)}" y="${A - 10}" text-anchor="${i === 23 ? "end" : i === 0 ? "start" : "middle"}"
           font-size="11" fill="currentColor" opacity=".55">${i === 23 ? "agora" : (23 - i) + "h"}</text>`).join("");
  // Marca de grade, para o olho medir a altura sem contar pixel.
  const grade = [0.5, 1].map(f =>
    `<line x1="12" y1="${py(teto * f).toFixed(1)}" x2="${L - 12}" y2="${py(teto * f).toFixed(1)}"
           stroke="currentColor" stroke-width="1" opacity=".12" stroke-dasharray="3 4"/>
     <text x="${L - 12}" y="${(py(teto * f) - 4).toFixed(1)}" text-anchor="end" font-size="10"
           fill="currentColor" opacity=".45">${fmt(Math.round(teto * f))}</text>`).join("");''')
s=s.replace('''    <line x1="10" y1="${base}" x2="${L - 10}" y2="${base}" stroke="currentColor" stroke-width="1" opacity=".25"/>''',
'''    ${grade}
    <line x1="12" y1="${base}" x2="${L - 12}" y2="${base}" stroke="currentColor" stroke-width="1" opacity=".25"/>''')
s=s.replace('''    <text x="10" y="${topo + 2}" font-size="10.5" fill="currentColor" opacity=".55">pico ${fmt(teto)}/h</text>''',
'''    <text x="12" y="${topo - 4}" font-size="11" fill="currentColor" opacity=".55">operações por hora</text>''')

# larguras por carta
s=s.replace('''          { rotulo:130, titulo:"Operações mais pedidas" }))}''',
            '''          { rotulo:112, titulo:"Operações mais pedidas" }))}''')
s=s.replace('''          texto: `${fmt(t.registros)} · ${fmtBytes(t.bytes)}` })), { rotulo:190 }))}''',
            '''          texto: `${fmt(t.registros)} · ${fmtBytes(t.bytes)}` })), { rotulo:170, texto:96 }))}''')
s=s.replace('''          texto: fmt(i.acessos) })), { rotulo:130 }))}''',
            '''          texto: fmt(i.acessos) })), { rotulo:118 }))}''')
s=s.replace('''          texto: `${b.tabelas} tab · ${fmt(b.registros)} reg`, cor:"var(--reg)" })), { rotulo:150 }))}''',
            '''          texto: `${b.tabelas} tab · ${fmt(b.registros)} reg`, cor:"var(--reg)" })), { rotulo:110, texto:96 }))}''')
s=s.replace('''        barras(d.usuarios_ativos.map(u => ({ nome:u.usuario, valor:u.acessos, cor:"var(--memo)" })),
          { rotulo:130 }))}''',
            '''        barras(d.usuarios_ativos.map(u => ({ nome:u.usuario, valor:u.acessos, cor:"var(--memo)" })),
          { rotulo:112 }))}''')

# plural certo nos rotulos
s=s.replace('''    kpi(fmt(s.usuarios), "usuários", `${fmt(d.por_nivel.length)} níveis`),''',
            '''    kpi(fmt(s.usuarios), "usuários",
        d.por_nivel.length === 1 ? "1 nível" : `${d.por_nivel.length} níveis`),''')
s=s.replace('''    kpi(fmt(s.bancos), "bancos", `${fmt(s.tabelas)} tabelas`),''',
            '''    kpi(fmt(s.bancos), "bancos", s.tabelas === 1 ? "1 tabela" : `${fmt(s.tabelas)} tabelas`),''')
s=s.replace('''    kpi(fmt(s.conexoes), "conexões", `${fmt(s.sessoes_web)} sessões web`, "viva"),''',
            '''    kpi(fmt(s.conexoes), "conexões",
        s.sessoes_web === 1 ? "1 sessão web" : `${fmt(s.sessoes_web)} sessões web`, "viva"),''')
open(p,'w').write(s)
print('escalas corrigidas')
