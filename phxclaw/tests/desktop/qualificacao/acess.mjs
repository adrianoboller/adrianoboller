// Uso: PLAYWRIGHT_BROWSERS_PATH=/opt/pw-browsers node tests/desktop/qualificacao/acess.mjs [--ui DIR]; saida em tests/desktop/out/qualificacao/.
// Teclado e acessibilidade: arvore AX (CDP), ordem de Tab, foco visivel, landmarks, aria-live,
// fontes reais por no (CSS.getPlatformFontsForNode).
import { readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { subir } from './servidor.mjs';
import { chromium, UI, OUT, CAP, MEDIR, GRADE, CONFIG } from './comum.mjs';
import { stubTauriFn } from './stub.mjs';
const grade = GRADE;
const { srv, porta } = await subir(UI, { config: CONFIG });
const ORIG = `http://127.0.0.1:${porta}`;
const TELAS = ['geral', 'agentes', 'ide', 'ferramentas', 'absorcao', 'tarefas', 'config'];
const out = { telas: {}, splash: {}, fontes: {} };
const browser = await chromium.launch();

const descreve = () => {
  const e = document.activeElement;
  if (!e || e === document.body) return null;
  const sel = e.id ? `${e.tagName.toLowerCase()}#${e.id}` : e.tagName.toLowerCase() + (e.classList.length ? '.' + [...e.classList].slice(0, 2).join('.') : '');
  const cs = getComputedStyle(e);
  const r = e.getBoundingClientRect();
  const nome = (e.getAttribute('aria-label') || e.textContent || e.placeholder || e.value || '').trim().replace(/\s+/g, ' ').slice(0, 30);
  const base = e.__base || {};
  const ind = { outline: cs.outlineStyle !== 'none' && parseFloat(cs.outlineWidth) > 0 ? `${cs.outlineStyle} ${cs.outlineWidth} ${cs.outlineColor}` : '', sombra: cs.boxShadow !== 'none' && cs.boxShadow !== base.sombra ? cs.boxShadow.slice(0, 40) : '', borda: base.borda && cs.borderTopColor !== base.borda ? cs.borderTopColor : '', fundo: base.fundo && cs.backgroundColor !== base.fundo ? cs.backgroundColor : '' };
  let visivel = r.width > 0 && r.height > 0;
  for (let n = e; n && n.nodeType === 1; n = n.parentElement) { const c = getComputedStyle(n); if (c.visibility === 'hidden' || +c.opacity === 0 || c.display === 'none') visivel = false; }
  const naTela = r.bottom > 0 && r.top < innerHeight && r.right > 0 && r.left < innerWidth;
  const ariaHidden = !!e.closest('[aria-hidden="true"]');
  return { k: e.__k ?? ('x' + Math.random()), sel, nome, visivel, naTela, ariaHidden, ind, temInd: !!(ind.outline || ind.sombra || ind.borda || ind.fundo), w: Math.round(r.width), h: Math.round(r.height) };
};
const marcarBase = () => {
  for (const e of document.querySelectorAll('button,a[href],input,select,textarea,[tabindex],canvas')) {
    const cs = getComputedStyle(e); e.__k ??= (window.__kc = (window.__kc || 0) + 1); e.__base = { sombra: cs.boxShadow, borda: cs.borderTopColor, fundo: cs.backgroundColor };
  }
  const vis = e => { const r = e.getBoundingClientRect(); if (!(r.width > 0 && r.height > 0)) return false; for (let n = e; n && n.nodeType === 1; n = n.parentElement) { const c = getComputedStyle(n); if (c.visibility === 'hidden' || +c.opacity === 0 || c.display === 'none') return false; } return true; };
  return [...document.querySelectorAll('button,a[href],input,select,textarea,[tabindex]')].filter(e => e.tabIndex >= 0 && !e.disabled && vis(e)).length;
};

async function percorrer(page, max = 260) {
  const n = await page.evaluate(marcarBase);
  await page.evaluate(() => { document.activeElement?.blur(); window.scrollTo(0, 0); });
  await page.mouse.click(1, 1).catch(() => {}); // ponto de partida do Tab: canto do topo, antes de todo controle
  await page.evaluate(() => document.activeElement?.blur());
  const seq = [];
  const vistos = new Set();
  let preso = 0;
  for (let i = 0; i < max; i++) {
    await page.keyboard.press('Tab');
    const d = await page.evaluate(descreve);
    if (!d) { seq.push(null); continue; }
    const k = d.k;
    if (seq.length && seq[seq.length - 1] && seq[seq.length - 1].k === k) preso++;
    if (vistos.has(k) && seq.length > 3) { seq.push(d); break; }
    vistos.add(k); seq.push(d);
  }
  return { focaveisVisiveis: n, alcancados: vistos.size, preso, seq };
}

async function ax(page) {
  const cdp = await page.context().newCDPSession(page);
  const { nodes } = await cdp.send('Accessibility.getFullAXTree');
  const val = x => x?.value ?? '';
  const roles = {};
  const semNome = [], soSimbolo = [], imgs = [], landmarks = [], vivos = [];
  for (const n of nodes) {
    if (n.ignored) continue;
    const role = val(n.role), nome = String(val(n.name)).trim();
    roles[role] = (roles[role] || 0) + 1;
    if (['button', 'link', 'textbox', 'searchbox', 'combobox', 'checkbox', 'tab', 'menuitem', 'spinbutton', 'radio', 'switch', 'slider'].includes(role)) {
      if (!nome) semNome.push(role);
      else if (!/[\p{L}\p{N}]/u.test(nome)) soSimbolo.push(`${role}:"${nome}"`);
    }
    if (role === 'image' || role === 'img') imgs.push(nome || '(sem nome)');
    if (['banner', 'navigation', 'main', 'contentinfo', 'complementary', 'region', 'search', 'form'].includes(role)) landmarks.push(`${role}${nome ? ':' + nome : ''}`);
    const live = (n.properties || []).find(p => p.name === 'live');
    if (live && val(live.value) !== 'off') vivos.push(`${role}:${nome.slice(0, 30)}(${val(live.value)})`);
  }
  await cdp.detach();
  return { semNome, soSimbolo, imgs, landmarks: [...new Set(landmarks)], vivos: [...new Set(vivos)], nBotoes: roles.button || 0 };
}

for (const [w, h] of [[1366, 768], [390, 844]]) {
  const ctx = await browser.newContext({ viewport: { width: w, height: h }, locale: 'pt-BR', isMobile: w === 390, hasTouch: w === 390 });
  await ctx.addInitScript(stubTauriFn, grade);
  await ctx.addInitScript(() => { try { localStorage.setItem('phxclaw.token', 'token-de-teste'); } catch {} });
  const page = await ctx.newPage();
  // splash: para onde vai o Tab enquanto o splash cobre a tela?
  if (w === 1366) {
    await page.goto(`${ORIG}/index.html`);
    await page.waitForTimeout(800);
    const seq = [];
    for (let i = 0; i < 6; i++) { await page.keyboard.press('Tab'); seq.push(await page.evaluate(descreve)); }
    out.splash = { seq, axDuranteSplash: await ax(page) };
    await page.waitForTimeout(3500);
    out.splash.depois = await page.evaluate(() => ({ appVisivel: document.getElementById('app').classList.contains('visible'), foco: document.activeElement?.tagName }));
  }
  await page.goto(`${ORIG}/index.html?screen=dashboard`);
  await page.waitForTimeout(1000);
  for (const tela of TELAS) {
    await page.evaluate(t => { location.hash = t; }, tela);
    await page.waitForTimeout(700);
    if (tela === 'ide') { await page.click('#ideAbrirBash'); await page.waitForTimeout(400); }
    const r = { ax: await ax(page), tab: await percorrer(page) };
    if (tela === 'ide') {
      // o canvas do terminal prende o Tab?
      await page.focus('#termCanvas');
      await page.keyboard.press('Tab');
      r.tabNoTerminal = await page.evaluate(() => document.activeElement?.id || document.activeElement?.tagName);
    }
    out.telas[`${tela}@${w}`] = r;
    const semInd = r.tab.seq.filter(d => d && !d.temInd);
    const invis = r.tab.seq.filter(d => d && (!d.visivel || d.ariaHidden));
    console.log(w, tela, 'focaveis', r.tab.focaveisVisiveis, 'alcancados', r.tab.alcancados, 'semIndicador', semInd.length, 'invisivel/ariaHidden', invis.length, 'semNome', r.ax.semNome.length, 'soSimbolo', r.ax.soSimbolo.length, 'live', r.ax.vivos.length);
  }
  if (w === 1366) {
    // fontes que de fato desenham
    const cdp = await ctx.newCDPSession(page);
    await cdp.send('DOM.enable'); await cdp.send('CSS.enable');
    const { root } = await cdp.send('DOM.getDocument', { depth: -1 });
    await page.evaluate(() => { location.hash = 'geral'; }); await page.waitForTimeout(500);
    for (const s of ['.nav.active span', '.nav.active em', '.hero-row h1', '.eyebrow', '.metric strong', '.footerbar span', '.footerbar b', '.top-actions button', '.brand-mini strong', '#eventConsole .event-row b', '.cap-list span', '.macro-lista span']) {
      const { nodeId } = await cdp.send('DOM.querySelector', { nodeId: root.nodeId, selector: s });
      if (!nodeId) { out.fontes[s] = 'nao achado'; continue; }
      const f = await cdp.send('CSS.getPlatformFontsForNode', { nodeId });
      out.fontes[s] = f.fonts.map(x => `${x.familyName}(${x.glyphCount}${x.isCustomFont ? ',web' : ''})`).join(' + ');
    }
    out.documentFonts = await page.evaluate(async () => { await document.fonts.ready; return { status: document.fonts.status, faces: [...document.fonts].map(f => `${f.family} ${f.weight} ${f.status}`), exo2: document.fonts.check('16px "Exo 2"') }; });
  }
  await ctx.close();
}
writeFileSync(join(OUT, 'acess.json'), JSON.stringify(out, null, 1));
console.log('fontes', JSON.stringify(out.fontes, null, 1), JSON.stringify(out.documentFonts));
console.log('splash', JSON.stringify(out.splash.seq), JSON.stringify(out.splash.depois));
await browser.close(); srv.close();
