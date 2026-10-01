// Executa DENTRO da pagina (page.evaluate). Mede contraste, corte, sobreposicao, alvos de toque,
// fundos cheios, cores e tamanhos. Devolve um objeto serializavel.
(opts) => {
  // Duas formas de cor computada: rgb()/rgba() e color(srgb r g b / a), que e como o Chromium
  // devolve um color-mix() (a folha mistura tokens desde o tema claro). Ler so a primeira fazia
  // a camada misturada sumir da conta do fundo, calada.
  const COR = /rgba?\([^)]+\)|color\(srgb [^)]+\)/g;
  const parse = s => {
    if (!s) return null;
    let m = s.match(/rgba?\(([^)]+)\)/);
    if (m) {
      const p = m[1].split(/[ ,/]+/).filter(Boolean).map(Number);
      return { r: p[0], g: p[1], b: p[2], a: p.length > 3 ? p[3] : 1 };
    }
    m = s.match(/color\(srgb ([^)]+)\)/);
    if (!m) return null;
    const [cor, alfa] = m[1].split('/');
    const p = cor.trim().split(/\s+/).map(Number);
    return { r: p[0] * 255, g: p[1] * 255, b: p[2] * 255, a: alfa === undefined ? 1 : +alfa };
  };
  const lin = c => { c /= 255; return c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4; };
  const lum = c => 0.2126 * lin(c.r) + 0.7152 * lin(c.g) + 0.0722 * lin(c.b);
  const razao = (a, b) => { const x = lum(a), y = lum(b); return (Math.max(x, y) + 0.05) / (Math.min(x, y) + 0.05); };
  const sobre = (top, base) => { const a = top.a; return { r: top.r * a + base.r * (1 - a), g: top.g * a + base.g * (1 - a), b: top.b * a + base.b * (1 - a), a: 1 }; };
  const hex = c => '#' + [c.r, c.g, c.b].map(v => Math.round(v).toString(16).padStart(2, '0')).join('');
  const sel = e => {
    if (!e || e.nodeType !== 1) return '';
    let s = e.tagName.toLowerCase();
    if (e.id) return s + '#' + e.id;
    if (e.classList.length) s += '.' + [...e.classList].slice(0, 2).join('.');
    const p = e.parentElement;
    let ps = '';
    if (p) { ps = p.id ? `#${p.id}` : p.tagName.toLowerCase() + (p.classList.length ? '.' + [...p.classList].slice(0, 2).join('.') : ''); }
    return ps ? `${ps} > ${s}` : s;
  };
  const visivel = e => {
    if (!e.getClientRects().length) return false;
    let n = e;
    while (n && n.nodeType === 1) {
      const cs = getComputedStyle(n);
      if (cs.visibility === 'hidden' || cs.display === 'none' || +cs.opacity === 0) return false;
      n = n.parentElement;
    }
    const r = e.getBoundingClientRect();
    return r.width > 0 && r.height > 0;
  };
  const opacidade = e => { let o = 1; for (let n = e; n && n.nodeType === 1; n = n.parentElement) o *= +getComputedStyle(n).opacity; return o; };
  // Fundo efetivo: compoe as camadas dos ancestrais (do html ate o elemento). Gradiente entra
  // pela media das paradas de cor e marca o resultado como aproximado.
  const fundo = e => {
    const camadas = [];
    let aprox = false;
    for (let n = e; n && n.nodeType === 1; n = n.parentElement) {
      const cs = getComputedStyle(n);
      const bi = cs.backgroundImage;
      if (bi && bi !== 'none' && /gradient/.test(bi) && n.className !== 'noise') {
        const cores = [...bi.matchAll(COR)].map(m => parse(m[0])).filter(Boolean);
        // gradiente radial com transparent ocupa so uma regiao: ignora as paradas transparentes
        const opacas = cores.filter(c => c.a > 0);
        if (opacas.length) {
          const med = opacas.reduce((a, c) => ({ r: a.r + c.r / opacas.length, g: a.g + c.g / opacas.length, b: a.b + c.b / opacas.length, a: a.a + c.a / opacas.length }), { r: 0, g: 0, b: 0, a: 0 });
          camadas.push(med); aprox = true;
        }
      }
      const bc = parse(cs.backgroundColor);
      if (bc && bc.a > 0) camadas.push(bc);
      if (bc && bc.a >= 1) break;
    }
    // O fundo do html (o --fundo do tema da pagina: #010418 no escuro, #f7f5f2 no claro).
    let base = parse(getComputedStyle(document.documentElement).backgroundColor) || { r: 1, g: 4, b: 24, a: 1 };
    base = { ...base, a: 1 };
    for (let i = camadas.length - 1; i >= 0; i--) base = sobre(camadas[i], base);
    return { cor: base, aprox };
  };

  const out = { contraste: [], cortado: [], sobreposto: [], toque: [], fundoCheio: [], cores: {}, tamanhos: {}, familias: {}, textos: [], scroll: {} };
  const de = document.documentElement;
  out.scroll = { docScrollW: de.scrollWidth, docClientW: de.clientWidth, bodyScrollW: document.body.scrollWidth, innerW: innerWidth, innerH: innerHeight, docScrollH: de.scrollHeight };

  // Elementos com texto direto
  const comTexto = [];
  const walker = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT);
  for (let t = walker.nextNode(); t; t = walker.nextNode()) {
    if (!t.nodeValue.trim()) continue;
    const e = t.parentElement;
    if (!e || ['SCRIPT', 'STYLE', 'OPTION'].includes(e.tagName)) continue;
    if (!visivel(e)) continue;
    const rg = document.createRange(); rg.selectNodeContents(t);
    const rects = [...rg.getClientRects()].filter(r => r.width > 0 && r.height > 0);
    if (!rects.length) continue;
    comTexto.push({ t, e, rects });
  }
  // inputs com valor/placeholder tambem sao texto
  for (const e of document.querySelectorAll('input:not([type=checkbox]):not([type=radio]),textarea,select')) {
    if (!visivel(e)) continue;
    comTexto.push({ t: null, e, rects: [e.getBoundingClientRect()], campo: true });
  }

  const vistos = new Map();
  for (const { t, e, rects, campo } of comTexto) {
    const cs = getComputedStyle(e);
    const r0 = rects[0];
    // so o que esta dentro da janela conta para contraste (fora dela ninguem ve sem rolar,
    // mas rolar pode trazer: conta tudo que esta no DOM visivel)
    let cor = parse(cs.color);
    const texto = campo ? (e.value || e.placeholder || '') : t.nodeValue.trim();
    if (!texto) continue;
    out.textos.push(texto);
    const tam = parseFloat(cs.fontSize), peso = +cs.fontWeight;
    out.tamanhos[tam] = (out.tamanhos[tam] || 0) + 1;
    const fam = cs.fontFamily.split(',')[0].trim();
    out.familias[fam] = (out.familias[fam] || 0) + 1;
    if (!cor || (cor.a === 0 && /text/.test(cs.webkitBackgroundClip + cs.backgroundClip))) continue;
    const op = opacidade(e);
    const f = fundo(e);
    cor = sobre({ ...cor, a: cor.a * op }, f.cor);
    const rz = razao(cor, f.cor);
    const grande = tam >= 24 || (tam >= 18.66 && peso >= 700);
    const minimo = grande ? 3 : 4.5;
    const chave = sel(e) + '|' + hex(cor) + '|' + hex(f.cor);
    out.cores[hex(parse(cs.color))] = (out.cores[hex(parse(cs.color))] || 0) + 1;
    // Controle inativo (disabled) e excecao do proprio WCAG 1.4.3: nao conta.
    if (rz < minimo && !e.closest(':disabled')) {
      const v = vistos.get(chave);
      if (v) { v.n++; continue; }
      const item = { sel: sel(e), texto: texto.slice(0, 40), cor: hex(cor), fundo: hex(f.cor), razao: +rz.toFixed(2), minimo, px: tam, peso, aprox: f.aprox, n: 1 };
      vistos.set(chave, item); out.contraste.push(item);
    }
    // corte: texto que passa da caixa de um ancestral com overflow escondido
    if (!campo) {
      const rr = rects.reduce((a, r) => ({ l: Math.min(a.l, r.left), t: Math.min(a.t, r.top), r: Math.max(a.r, r.right), b: Math.max(a.b, r.bottom) }), { l: 1e9, t: 1e9, r: -1e9, b: -1e9 });
      for (let n = e; n && n.nodeType === 1; n = n.parentElement) {
        const c = getComputedStyle(n);
        if (c.overflowX === 'visible' && c.overflowY === 'visible') continue;
        const br = (n === de || n === document.body) ? { left: 0, top: 0, right: innerWidth, bottom: innerHeight } : n.getBoundingClientRect();
        const xEsc = (c.overflowX === 'hidden' || c.overflowX === 'clip');
        const yEsc = (c.overflowY === 'hidden' || c.overflowY === 'clip');
        const foraX = rr.r > br.right + 1 || rr.l < br.left - 1;
        const foraY = rr.b > br.bottom + 1 || rr.t < br.top - 1;
        if ((foraX && xEsc) || (foraY && yEsc)) {
          out.cortado.push({ sel: sel(e), texto: texto.slice(0, 40), por: sel(n), eixo: (foraX && xEsc ? 'x' : '') + (foraY && yEsc ? 'y' : ''), elipse: c.textOverflow === 'ellipsis' || cs.textOverflow === 'ellipsis' });
          break;
        }
        if (c.overflowX !== 'visible' || c.overflowY !== 'visible') {
          // conteiner que rola: o texto e alcancavel; para de subir so se ele contem
          if (!foraX && !foraY) continue;
          break;
        }
      }
      // so a janela: sai da viewport e o html nao rola
      if ((rr.r > innerWidth + 1 || rr.b > innerHeight + 1) && getComputedStyle(de).overflow === 'hidden') {
        // ja coberto acima na maioria dos casos
      }
    }
  }
  // sobreposicao entre textos diferentes (so o que esta na janela)
  const recorta = (e, r) => { let a = { left: r.left, top: r.top, right: r.right, bottom: r.bottom }; for (let n = e.parentElement; n && n.nodeType === 1; n = n.parentElement) { const c = getComputedStyle(n); if (c.overflowX === 'visible' && c.overflowY === 'visible') continue; const b = (n === de || n === document.body) ? { left: 0, top: 0, right: innerWidth, bottom: innerHeight } : n.getBoundingClientRect(); a = { left: Math.max(a.left, b.left), top: Math.max(a.top, b.top), right: Math.min(a.right, b.right), bottom: Math.min(a.bottom, b.bottom) }; } return a; };
  for (const x of comTexto) x.rects = x.rects.map(r => recorta(x.e, r)).filter(r => r.right - r.left > 1 && r.bottom - r.top > 1);
  const naJanela = comTexto.filter(x => !x.campo && x.rects.length).map(x => ({ ...x, r: x.rects[0] })).filter(x => x.r.bottom > 0 && x.r.top < innerHeight && x.r.right > 0 && x.r.left < innerWidth);
  for (let i = 0; i < naJanela.length && out.sobreposto.length < 60; i++) {
    for (let j = i + 1; j < naJanela.length; j++) {
      const A = naJanela[i], B = naJanela[j];
      if (A.e.contains(B.e) || B.e.contains(A.e)) continue;
      for (const a of A.rects) for (const b of B.rects) {
        const w = Math.min(a.right, b.right) - Math.max(a.left, b.left);
        const h = Math.min(a.bottom, b.bottom) - Math.max(a.top, b.top);
        if (w > 2 && h > 2) {
          // descarta o que esta escondido por clip (um dos dois fora da caixa do ancestral)
          const topo = document.elementFromPoint(Math.max(a.left, b.left) + w / 2, Math.max(a.top, b.top) + h / 2);
          out.sobreposto.push({ a: sel(A.e), ta: A.t.nodeValue.trim().slice(0, 25), b: sel(B.e), tb: B.t.nodeValue.trim().slice(0, 25), area: Math.round(w * h), topo: topo ? sel(topo) : '' });
          break;
        }
      }
    }
  }
  // alvos de toque
  const interativos = [...document.querySelectorAll('button,a[href],input,select,textarea,summary,[role=button],[tabindex]:not([tabindex="-1"])')].filter(visivel);
  out.nInterativos = interativos.length;
  for (const e of interativos) {
    const r = e.getBoundingClientRect();
    if (r.width < 44 || r.height < 44) out.toque.push({ sel: sel(e), nome: (e.getAttribute('aria-label') || e.textContent || e.placeholder || '').trim().slice(0, 25), w: Math.round(r.width), h: Math.round(r.height), menor24: r.width < 24 || r.height < 24 });
  }
  // botoes com fundo cheio (estado normal, sem hover)
  for (const e of document.querySelectorAll('button,[role=button],.acao')) {
    if (!visivel(e)) continue;
    const cs = getComputedStyle(e);
    const bc = parse(cs.backgroundColor);
    const grad = cs.backgroundImage !== 'none';
    if ((bc && bc.a > 0.3) || grad) out.fundoCheio.push({ sel: sel(e), texto: (e.textContent || '').trim().slice(0, 25), fundo: grad ? 'gradiente' : hex(bc) + (bc.a < 1 ? `@${bc.a}` : ''), classe: e.className });
  }
  // tons de acao
  out.acoes = [...document.querySelectorAll('.acao')].filter(visivel).map(e => ({ texto: e.textContent.trim().slice(0, 30), classe: [...e.classList].filter(c => c !== 'acao').join(' '), cor: hex(parse(getComputedStyle(e).color)), borda: hex(parse(getComputedStyle(e).borderTopColor)) }));
  // rolagem horizontal interna e conteineres que cortam conteudo
  out.conteineres = [];
  for (const e of document.querySelectorAll('body *')) {
    if (!visivel(e)) continue;
    const cs = getComputedStyle(e);
    if (e.scrollWidth > e.clientWidth + 2 && e.clientWidth > 0 && (cs.overflowX === 'auto' || cs.overflowX === 'scroll')) out.conteineres.push({ sel: sel(e), tipo: 'rola-x', sw: e.scrollWidth, cw: e.clientWidth });
    if (e.scrollHeight > e.clientHeight + 2 && e.clientHeight > 0 && (cs.overflowY === 'hidden') && e.children.length && e.scrollHeight - e.clientHeight > 20) out.conteineres.push({ sel: sel(e), tipo: 'corta-y', sh: e.scrollHeight, ch: e.clientHeight });
  }
  out.textos = [...new Set(out.textos)];
  return out;
}
