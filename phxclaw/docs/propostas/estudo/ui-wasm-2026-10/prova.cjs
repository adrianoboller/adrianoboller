// Prova de CSP, funcao, a11y e teclado das 6 telas. uso: node prova.cjs
const { chromium } = require('/opt/node22/lib/node_modules/playwright');
const http = require('http'), fs = require('fs'), path = require('path');
const RAIZ = path.join(__dirname, 'out');
const CSPS = {
  estrita: "default-src 'self'; script-src 'self'; style-src 'self'; object-src 'none'; base-uri 'none'; frame-ancestors 'none'",
  wasm: "default-src 'self'; script-src 'self' 'wasm-unsafe-eval'; style-src 'self'; object-src 'none'; base-uri 'none'; frame-ancestors 'none'",
};
let csp = '';
const tipo = { '.js': 'text/javascript', '.wasm': 'application/wasm', '.html': 'text/html' };
const srv = http.createServer((q, r) => {
  const f = path.join(RAIZ, decodeURIComponent(q.url.split('?')[0]));
  if (!f.startsWith(RAIZ) || !fs.existsSync(f) || fs.statSync(f).isDirectory()) { r.writeHead(404); return r.end(); }
  r.writeHead(200, { 'content-type': tipo[path.extname(f)] || 'application/octet-stream', 'content-security-policy': csp });
  fs.createReadStream(f).pipe(r);
}).listen(0);
(async () => {
  const porta = srv.address().port;
  const nav = await chromium.launch();
  const apps = ['vanilla', 'react', 'leptos', 'yew', 'dioxusmin', 'dioxus'];
  const res = [];
  for (const [nomeCsp, valor] of (process.env.SO_TEMPO ? [] : Object.entries(CSPS))) for (const a of apps) {
    csp = valor;
    const tempos = []; let linha = {};
    for (let rodada = 0; rodada < 7; rodada++) {
      const ctx = await nav.newContext(); const p = await ctx.newPage();
      const viol = [], erros = [];
      await p.exposeFunction('__viol', v => viol.push(v));
      await p.addInitScript(() => document.addEventListener('securitypolicyviolation', e => window.__viol(e.violatedDirective + ' ' + e.blockedURI)));
      p.on('pageerror', e => erros.push(String(e.message).slice(0, 120)));
      const t0 = Date.now();
      await p.goto(`http://127.0.0.1:${porta}/${a}/index.html`);
      let ok = true;
      try { await p.waitForSelector('tbody tr', { timeout: 4000 }); } catch { ok = false; }
      const ms = await p.evaluate(() => performance.now());
      if (ok) tempos.push(await p.evaluate(() => performance.getEntriesByType('navigation')[0] ? performance.now() : 0));
      if (rodada === 0) {
        linha = { app: a, csp: nomeCsp, montou: ok, violacoes: [...new Set(viol)], erros: [...new Set(erros)] };
        if (ok) {
          linha.linhas = await p.locator('tbody tr').count();
          await p.getByLabel('Filtrar').fill('00');
          await p.waitForTimeout(100);
          linha.filtradas = await p.locator('tbody tr').count();
          await p.getByRole('button', { name: 'Salvar' }).click();
          await p.waitForTimeout(100);
          linha.status = await p.getByRole('status').textContent();
          await p.focus('body'); await p.evaluate(() => document.activeElement && document.activeElement.blur());
          const ordem = [];
          for (let i = 0; i < 9; i++) { await p.keyboard.press('Tab'); ordem.push(await p.evaluate(() => { const e = document.activeElement; return e.tagName + (e.getAttribute('aria-label') ? '[' + e.getAttribute('aria-label') + ']' : '') + (e.parentElement.tagName === 'LABEL' ? '<' + e.parentElement.firstChild.textContent : ''); })); }
          linha.tab = ordem.join(' > ');
          linha.aria = (await p.locator('body').ariaSnapshot()).length;
          linha.ariaTxt = await p.locator('body').ariaSnapshot();
        }
      }
      await ctx.close();
    }
    // tempo ate a primeira linha: refeito por carimbo dentro da pagina
    res.push(linha);
  }
  // tempo medido a parte: ms do navigationStart ate tbody tr existir, mediana de 7, CSP wasm
  csp = CSPS.wasm;
  for (const a of apps) {
    const ts = [];
    for (let i = 0; i < 15; i++) {
      const ctx = await nav.newContext(); const p = await ctx.newPage();
      await p.addInitScript(() => { new MutationObserver((m, o) => { if (document.querySelector('tbody tr')) { window.__t = performance.now(); o.disconnect(); } }).observe(document, { childList: true, subtree: true }); });
      await p.goto(`http://127.0.0.1:${porta}/${a}/index.html`);
      try { await p.waitForFunction(() => window.__t, null, { timeout: 5000 }); ts.push(await p.evaluate(() => window.__t)); } catch { ts.push(NaN); }
      await ctx.close();
    }
    ts.sort((x, y) => x - y);
    res.push({ app: a, n: ts.length, min: +ts[0].toFixed(1), med: +ts[ts.length>>1].toFixed(1), max: +ts[ts.length-1].toFixed(1), nav: nav.version() });
  }
  await nav.close(); srv.close();
  const b0 = res.find(r => r.app === 'vanilla' && r.ariaTxt); const base = b0 && b0.ariaTxt;
  for (const r of res) { if (r.ariaTxt !== undefined) { r.aria_igual_vanilla = r.ariaTxt === base; delete r.ariaTxt; } }
  fs.writeFileSync(path.join(__dirname, process.env.SO_TEMPO ? 'tempo.json' : 'prova.json'), JSON.stringify(res, null, 1));
  for (const r of res) console.log(JSON.stringify(r));
})();
