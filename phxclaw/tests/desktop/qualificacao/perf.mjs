// Uso: PLAYWRIGHT_BROWSERS_PATH=/opt/pw-browsers node tests/desktop/qualificacao/perf.mjs [--ui DIR]; saida em tests/desktop/out/qualificacao/.
// Desempenho percebido: FCP, tela util (app visivel), bytes por recurso, tarefas longas,
// custo do phx-grid no caminho critico. Desktop sem limitacao e celular com CPU 4x + rede 4G lenta.
import { readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { gzipSync } from 'node:zlib';
import { subir } from './servidor.mjs';
import { chromium, UI, OUT, CAP, MEDIR, GRADE, CONFIG } from './comum.mjs';
const { srv, porta } = await subir(UI, { config: CONFIG });
const ORIG = `http://127.0.0.1:${porta}`;
const browser = await chromium.launch();
const out = {};
async function medir(nome, { w, h, cpu = 1, rede = null, url, bloquearGrid = false, rep = 3 }) {
  const runs = [];
  for (let i = 0; i < rep; i++) {
    const ctx = await browser.newContext({ viewport: { width: w, height: h }, serviceWorkers: 'block' });
    await ctx.addInitScript(() => {
      window.__lt = [];
      try { new PerformanceObserver(l => { for (const e of l.getEntries()) window.__lt.push({ ini: Math.round(e.startTime), dur: Math.round(e.duration), nome: e.attribution?.[0]?.containerSrc || e.name }); }).observe({ type: 'longtask', buffered: true }); } catch {}
      try { localStorage.setItem('phxclaw.token', 't'); } catch {}
      const t0 = performance.now();
      const obs = new MutationObserver(() => { const a = document.getElementById('app'); if (a && a.classList.contains('visible') && !window.__util) window.__util = performance.now(); });
      document.addEventListener('DOMContentLoaded', () => obs.observe(document.getElementById('app'), { attributes: true }));
    });
    const page = await ctx.newPage();
    const cdp = await ctx.newCDPSession(page);
    await cdp.send('Network.enable');
    if (cpu > 1) await cdp.send('Emulation.setCPUThrottlingRate', { rate: cpu });
    if (rede) await cdp.send('Network.emulateNetworkConditions', { offline: false, ...rede });
    const bytes = {};
    cdp.on('Network.loadingFinished', e => { bytes[e.requestId] = e.encodedDataLength; });
    const urls = {};
    cdp.on('Network.requestWillBeSent', e => { urls[e.requestId] = new URL(e.request.url).pathname; });
    if (bloquearGrid) await page.route(/phx-grid\.js$/, r => r.fulfill({ status: 200, contentType: 'text/javascript', body: 'window.PhxGrid=null;' }));
    await page.goto(`${ORIG}${url}`, { waitUntil: 'load' });
    await page.waitForTimeout(url.includes('dashboard') ? 1500 : 4500);
    const m = await page.evaluate(() => {
      const nav = performance.getEntriesByType('navigation')[0];
      const fcp = performance.getEntriesByName('first-contentful-paint')[0]?.startTime;
      return { fcp: Math.round(fcp || 0), dcl: Math.round(nav.domContentLoadedEventEnd), load: Math.round(nav.loadEventEnd), util: Math.round(window.__util || 0), longtasks: window.__lt };
    });
    m.bytes = Object.entries(bytes).map(([id, b]) => [urls[id], b]).sort((a, b) => b[1] - a[1]);
    m.total = m.bytes.reduce((a, x) => a + x[1], 0);
    runs.push(m);
    await ctx.close();
  }
  const med = k => runs.map(r => r[k]).sort((a, b) => a - b)[Math.floor(runs.length / 2)];
  out[nome] = { fcp: med('fcp'), dcl: med('dcl'), load: med('load'), util: med('util'), total: med('total'), faixa: { fcp: [Math.min(...runs.map(r => r.fcp)), Math.max(...runs.map(r => r.fcp))], dcl: [Math.min(...runs.map(r => r.dcl)), Math.max(...runs.map(r => r.dcl))] }, longtasks: runs[0].longtasks, bytes: runs[0].bytes.slice(0, 12) };
  console.log(nome, JSON.stringify({ fcp: out[nome].fcp, dcl: out[nome].dcl, load: out[nome].load, util: out[nome].util, total: out[nome].total, faixa: out[nome].faixa, lt: out[nome].longtasks }));
}
const REDE4G = { latency: 150, downloadThroughput: 1.6 * 1024 * 1024 / 8, uploadThroughput: 750 * 1024 / 8 };
await medir('desktop_boot_com_splash', { w: 1366, h: 768, url: '/index.html' });
await medir('desktop_direto', { w: 1366, h: 768, url: '/index.html?screen=dashboard' });
await medir('desktop_direto_sem_grid', { w: 1366, h: 768, url: '/index.html?screen=dashboard', bloquearGrid: true });
await medir('celular_tarefas_pwa', { w: 390, h: 844, cpu: 4, rede: REDE4G, url: '/?tela=tarefas' });
await medir('celular_direto', { w: 390, h: 844, cpu: 4, rede: REDE4G, url: '/index.html?screen=dashboard' });
await medir('celular_direto_sem_grid', { w: 390, h: 844, cpu: 4, rede: REDE4G, url: '/index.html?screen=dashboard', bloquearGrid: true });
// tamanhos brutos e gzip
const arqs = ['assets/vendor/phx-grid/phx-grid.js', 'assets/vendor/phx-grid/phx-grid.css', 'assets/config-catalogo.json', 'assets/equipe.json', 'assets/app.js', 'assets/app.css', 'assets/fonte/exo2-latin.woff2', 'index.html'];
out.gzip = Object.fromEntries(arqs.map(a => { const b = readFileSync(join(UI, a)); return [a, { bruto: b.length, gzip: gzipSync(b, { level: 9 }).length }]; }));
console.log(JSON.stringify(out.gzip));
writeFileSync(join(OUT, 'perf.json'), JSON.stringify(out, null, 1));
await browser.close(); srv.close();
