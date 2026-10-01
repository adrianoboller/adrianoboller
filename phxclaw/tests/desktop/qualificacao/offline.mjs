// Uso: PLAYWRIGHT_BROWSERS_PATH=/opt/pw-browsers node tests/desktop/qualificacao/offline.mjs [--ui DIR]; saida em tests/desktop/out/qualificacao/.
// PWA sem rede DE VERDADE: o setOffline do Playwright nao alcanca o fetch do service worker,
// entao a rede cai derrubando o servidor. Duas jornadas: (A) primeira visita e queda;
// (B) visita, recarga com rede (o SW ja controla e guarda o que passa), e queda.
import { readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { subir } from './servidor.mjs';
import { chromium, UI, OUT, CAP, MEDIR, GRADE, CONFIG } from './comum.mjs';
const TELAS = ['geral', 'agentes', 'ide', 'ferramentas', 'absorcao', 'tarefas', 'config'];
const browser = await chromium.launch();
const out = {};
for (const jornada of ['A_primeira_visita', 'B_segunda_visita']) {
  const { srv, porta } = await subir(UI, { config: CONFIG });
  const ORIG = `http://localhost:${porta}`;
  const ctx = await browser.newContext({ viewport: { width: 390, height: 844 }, locale: 'pt-BR', isMobile: true, hasTouch: true });
  await ctx.addInitScript(() => { try { localStorage.setItem('phxclaw.token', 'token-de-teste'); } catch {} });
  const page = await ctx.newPage();
  await page.goto(`${ORIG}/?tela=tarefas`);
  await page.evaluate(() => navigator.serviceWorker.ready);
  await page.waitForTimeout(1500);
  if (jornada.startsWith('B')) {
    await page.reload(); await page.waitForTimeout(1500);
    for (const t of TELAS) { await page.evaluate(x => { location.hash = x; }, t); await page.waitForTimeout(300); }
    await page.waitForTimeout(1000);
  }
  const cache = await page.evaluate(async () => { const r = []; for (const k of await caches.keys()) r.push(...(await (await caches.open(k)).keys()).map(q => new URL(q.url).pathname)); return r; });
  srv.closeAllConnections?.(); await new Promise(r => srv.close(r));
  const falhas = [];
  page.on('requestfailed', r => falhas.push(new URL(r.url()).pathname));
  page.on('response', r => { if (r.status() >= 400) falhas.push(new URL(r.url()).pathname + ' ' + r.status()); });
  await page.reload().catch(e => falhas.push('reload: ' + e.message.split('\n')[0]));
  await page.waitForTimeout(4500);
  const res = { cacheAntesDaQueda: cache.length, semCache: ['/assets/equipe.json', '/assets/ferramentas.json', '/assets/absorcao.json', '/assets/fonte/exo2-latin.woff2'].filter(x => !cache.includes(x)) };
  for (const t of TELAS) {
    await page.evaluate(x => { location.hash = x; }, t).catch(() => {});
    await page.waitForTimeout(700);
    res[t] = await page.evaluate(() => { const t = document.body.dataset.tela; return [...document.querySelectorAll(`section.tela[data-tela="${t}"] .tela-sub`)].map(e => e.textContent.trim()).join(' | '); }).catch(e => 'ERRO ' + e.message);
    await page.screenshot({ path: join(CAP, `offline_${jornada}_${t}.png`) });
  }
  res.fonte = await page.evaluate(async () => { await document.fonts.ready; return [...document.fonts].map(f => `${f.family} ${f.status}`).join(','); }).catch(() => '?');
  res.falhas = [...new Set(falhas)];
  out[jornada] = res;
  await ctx.close();
}
writeFileSync(join(OUT, 'offline.json'), JSON.stringify(out, null, 1));
console.log(JSON.stringify(out, null, 1));
await browser.close();
