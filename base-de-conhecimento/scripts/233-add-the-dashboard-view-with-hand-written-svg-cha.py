# Add the dashboard view with hand-written SVG charts
# 27/08 22:36

p='crates/phxsql-server/ui/index.html'
s=open(p).read()

graficos = '''// =====================================================================
// Gráficos do painel. SVG escrito à mão, como o resto do projeto: são
// barras, área e anel — e uma biblioteca de gráfico não diria mais nada
// do que isso diz, ao custo de uma dependência.
//
// Todos usam currentColor e as variáveis do tema, então trocam de cor com
// o sol/lua sem nenhuma linha a mais.
// =====================================================================

const fmt = n => (n ?? 0).toLocaleString("pt-BR");
function fmtBytes(b) {
  if (!b) return "0 B";
  const u = ["B","KB","MB","GB","TB"];
  const i = Math.min(u.length - 1, Math.floor(Math.log(b) / Math.log(1024)));
  return (b / Math.pow(1024, i)).toFixed(i ? 1 : 0).replace(".", ",") + " " + u[i];
}

/// Barras horizontais. Boa para nome longo, que é o caso de tabela e de IP.
function barras(itens, opc = {}) {
  if (!itens.length) return `<div class="vazioc">sem dados ainda</div>`;
  const alt = 26, teto = Math.max(...itens.map(i => i.valor), 1);
  const larguraRotulo = opc.rotulo ?? 150;
  const h = itens.length * alt + 8;
  const linhas = itens.map((it, k) => {
    const y = k * alt + 4;
    const w = Math.max(1, (it.valor / teto) * (620 - larguraRotulo - 70));
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
  return `<svg viewBox="0 0 620 ${h}" role="img" aria-label="${esc(opc.titulo || "gráfico de barras")}">${linhas}</svg>`;
}

/// Área ao longo do tempo. Vinte e quatro baldes de uma hora.
function areaHoras(serie, recusadas) {
  const L = 620, A = 140, base = A - 26, topo = 10;
  const teto = Math.max(...serie, 1);
  const px = i => (i / (serie.length - 1)) * (L - 20) + 10;
  const py = v => base - (v / teto) * (base - topo);
  const linha = serie.map((v, i) => `${px(i).toFixed(1)},${py(v).toFixed(1)}`).join(" ");
  const area = `10,${base} ${linha} ${(L - 10)},${base}`;
  const barrasRec = recusadas.map((v, i) =>
    v ? `<rect x="${(px(i) - 3).toFixed(1)}" y="${py(v).toFixed(1)}" width="6"
             height="${(base - py(v)).toFixed(1)}" fill="var(--log)" opacity=".75"/>` : "").join("");
  const marcas = [0, 6, 12, 18, 23].map(i =>
    `<text x="${px(i).toFixed(1)}" y="${A - 8}" text-anchor="middle" font-size="10"
           fill="currentColor" opacity=".55">${i === 23 ? "agora" : (23 - i) + "h atrás"}</text>`).join("");
  return `<svg viewBox="0 0 ${L} ${A}" role="img"
    aria-label="Operações por hora nas últimas 24 horas, com as recusadas destacadas">
    <line x1="10" y1="${base}" x2="${L - 10}" y2="${base}" stroke="currentColor" stroke-width="1" opacity=".25"/>
    <polygon points="${area}" fill="var(--laranja)" opacity=".16"/>
    <polyline points="${linha}" fill="none" stroke="var(--laranja)" stroke-width="2"
              stroke-linejoin="round" stroke-linecap="round"/>
    ${barrasRec}
    <text x="10" y="${topo + 2}" font-size="10.5" fill="currentColor" opacity=".55">pico ${fmt(teto)}/h</text>
    ${marcas}
  </svg>`;
}

/// Anel. Para composição — quantos usuários de cada nível.
function anel(fatias) {
  const total = fatias.reduce((a, f) => a + f.valor, 0);
  if (!total) return `<div class="vazioc">sem dados ainda</div>`;
  const R = 52, r = 33, cx = 70, cy = 70;
  let ang = -Math.PI / 2;
  const arcos = fatias.map(f => {
    const passo = (f.valor / total) * Math.PI * 2;
    const [a0, a1] = [ang, ang + passo];
    ang = a1;
    const grande = passo > Math.PI ? 1 : 0;
    const p = (raio, a) => `${(cx + raio * Math.cos(a)).toFixed(2)},${(cy + raio * Math.sin(a)).toFixed(2)}`;
    // Fatia única vira anel inteiro: o arco de 360° some, porque começa e
    // termina no mesmo ponto.
    if (fatias.length === 1 || passo >= Math.PI * 2 - 1e-6)
      return `<circle cx="${cx}" cy="${cy}" r="${(R + r) / 2}" fill="none"
                stroke="${f.cor}" stroke-width="${R - r}"/>`;
    return `<path d="M${p(R, a0)} A${R},${R} 0 ${grande},1 ${p(R, a1)}
              L${p(r, a1)} A${r},${r} 0 ${grande},0 ${p(r, a0)} Z" fill="${f.cor}"/>`;
  }).join("");
  const legenda = fatias.map((f, i) =>
    `<g transform="translate(150,${28 + i * 21})">
       <rect width="11" height="11" rx="2" fill="${f.cor}"/>
       <text x="18" y="10" font-size="12" fill="currentColor">${esc(f.nome)}</text>
       <text x="150" y="10" font-size="12" text-anchor="end"
             font-family="IBM Plex Mono, monospace" fill="currentColor" opacity=".7">${f.valor}</text>
     </g>`).join("");
  return `<svg viewBox="0 0 320 ${Math.max(145, 34 + fatias.length * 21)}" role="img"
    aria-label="Composição: ${fatias.map(f => f.nome + " " + f.valor).join(", ")}">
    ${arcos}
    <text x="${cx}" y="${cy + 1}" text-anchor="middle" font-size="20" font-weight="700"
          fill="currentColor">${total}</text>
    <text x="${cx}" y="${cy + 16}" text-anchor="middle" font-size="9.5"
          fill="currentColor" opacity=".55">TOTAL</text>
    ${legenda}
  </svg>`;
}

async function vPainel() {
  const d = await api("painel");
  const s = d.resumo;
  est.painel = d;

  const kpi = (v, r, u, classe) =>
    `<div class="kpi ${classe || ""}"><div class="v">${v}</div>
       <div class="r">${r}</div>${u ? `<div class="u">${u}</div>` : ""}</div>`;

  const kpis = [
    kpi(fmt(s.bancos), "bancos", `${fmt(s.tabelas)} tabelas`),
    kpi(fmt(s.registros), "registros", fmtBytes(s.bytes_reg) + " no .reg"),
    kpi(fmt(s.usuarios), "usuários", `${fmt(d.por_nivel.length)} níveis`),
    kpi(fmt(s.conexoes), "conexões", `${fmt(s.sessoes_web)} sessões web`, "viva"),
    kpi(fmt(s.acessos), "acessos", `${s.ms_medio} ms em média`),
    kpi(fmt(s.acessos_recusados), "recusados",
        s.acessos ? ((s.acessos_recusados / s.acessos) * 100).toFixed(1).replace(".", ",") + "% do total" : "",
        s.acessos_recusados ? "mal" : ""),
    kpi(fmt(s.bloqueios), "IPs bloqueados", "", s.bloqueios ? "mal" : ""),
    kpi(fmt(s.tabelas_em_ram), "em memória", fmtBytes(s.bytes_em_ram)),
  ].join("");

  const carta = (titulo, legenda, corpo, larga) =>
    `<div class="carta ${larga ? "larga" : ""}"><h4>${titulo}</h4>
       <p class="leg">${legenda}</p>${corpo}</div>`;

  const CORES = { admin:"var(--vermelhao)", dono:"var(--laranja)", operador:"var(--ambar)",
                  leitor:"var(--reg)", nenhum:"var(--texto-3)" };

  return `<div class="kpis">${kpis}</div><div class="cartas">
    ${carta("Operações por hora", "últimas 24 horas · as barras vermelhas são as recusadas",
        areaHoras(d.por_hora, d.recusadas_por_hora), true)}
    ${carta("Operações mais pedidas", "verde é o que passou, vermelho o que foi recusado",
        barras(d.por_operacao.map(o => ({ nome:o.op, valor:o.ok, segundo:o.recusados,
          texto: o.recusados ? `${fmt(o.ok)} + ${fmt(o.recusados)}✕` : fmt(o.ok) })),
          { rotulo:130, titulo:"Operações mais pedidas" }))}
    ${carta("Usuários por nível", "quem pode o quê, do config.json",
        anel(d.por_nivel.map(n => ({ nome:n.nivel, valor:n.quantos, cor:CORES[n.nivel] || "var(--texto-3)" }))))}
    ${carta("Maiores tabelas", "por registro · o tamanho é o do .reg",
        barras(d.maiores_tabelas.map(t => ({ nome:t.tabela, valor:t.registros,
          texto: `${fmt(t.registros)} · ${fmtBytes(t.bytes)}` })), { rotulo:190 }))}
    ${carta("De onde vêm", "por IP · vermelho é o que foi recusado",
        barras(d.top_ips.map(i => ({ nome:i.ip, valor:i.acessos - i.recusados, segundo:i.recusados,
          texto: fmt(i.acessos) })), { rotulo:130 }))}
    ${carta("Bancos", "tabelas por banco de dados",
        barras(d.bancos.map(b => ({ nome:b.nome, valor:b.tabelas,
          texto: `${b.tabelas} tab · ${fmt(b.registros)} reg`, cor:"var(--reg)" })), { rotulo:150 }))}
    ${carta("Quem mais usou", "por login, no log inteiro",
        barras(d.usuarios_ativos.map(u => ({ nome:u.usuario, valor:u.acessos, cor:"var(--memo)" })),
          { rotulo:130 }))}
  </div>`;
}

'''

s = s.replace('async function abrirAdmin(qual) {', graficos + 'async function abrirAdmin(qual) {')
s = s.replace('''  try {
    if (qual === "usuarios") {''','''  try {
    if (qual === "painel") {
      $("#titulo").textContent = "Painel";
      $("#subtitulo").textContent =
        `${est.painel ? "" : ""}o servidor inteiro numa tela · uma única chamada ao servidor`;
      p.innerHTML = await vPainel();
      return;
    }
    if (qual === "usuarios") {''')
s = s.replace('              teto:200, esquemaAtual:null, grade:null };',
              '              teto:200, esquemaAtual:null, grade:null, painel:null };')
open(p,'w').write(s)
print('painel ok')
