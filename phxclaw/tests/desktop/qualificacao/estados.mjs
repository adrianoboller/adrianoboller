// Uso: PLAYWRIGHT_BROWSERS_PATH=/opt/pw-browsers node tests/desktop/qualificacao/estados.mjs [--ui DIR]; saida em tests/desktop/out/qualificacao/.
// Estados: carregando, vazio, erro, sem JSON gerado, sem fabrica de idiomas, sem host, sem rede (PWA).
import { readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { subir } from './servidor.mjs';
import { chromium, UI, OUT, CAP, MEDIR, GRADE, CONFIG } from './comum.mjs';
import { stubTauriFn } from './stub.mjs';
const grade = GRADE;
const { srv, porta, modo } = await subir(UI, { config: CONFIG });
const ORIG = `http://127.0.0.1:${porta}`;
const TELAS = ['geral', 'agentes', 'ide', 'ferramentas', 'absorcao', 'tarefas', 'config'];
const browser = await chromium.launch();
const out = {};
const ler = page => page.evaluate(() => {
  const q = s => [...document.querySelectorAll(s)].filter(e => e.getClientRects().length && !e.closest('[hidden]')).map(e => e.textContent.trim().replace(/\s+/g, ' ').slice(0, 160)).filter(Boolean);
  const t = document.body.dataset.tela;
  const sec = document.querySelector(`section.tela[data-tela="${t}"]`);
  return {
    status: q(`section.tela[data-tela="${t}"] .tela-sub, section.tela[data-tela="${t}"] .aviso, section.tela[data-tela="${t}"] .vazio, section.tela[data-tela="${t}"] .phx-vazio, section.tela[data-tela="${t}"] .term-vazio`),
    metricas: t === 'geral' ? q('.metric small, .metric strong, .event-empty, #apiEndpoint') : [],
    linhas: sec ? sec.querySelectorAll('tbody tr').length : 0,
    topo: q('#nativeBridgeStatus, #liveEventStatus'),
  };
});
async function rodada(nome, { tauri = true, rotaAtraso = null, idioma = 'pt', largura = 1366, telas = TELAS, depois = null } = {}) {
  const ctx = await browser.newContext({ viewport: { width: largura, height: largura === 390 ? 844 : 768 }, locale: 'pt-BR' });
  if (tauri) await ctx.addInitScript(stubTauriFn, grade);
  await ctx.addInitScript(l => { try { localStorage.setItem('phxclaw.token', 'token-de-teste'); localStorage.setItem('phxclaw.idioma', l); } catch {} }, idioma);
  const page = await ctx.newPage();
  const erros = [];
  page.on('pageerror', e => erros.push(String(e)));
  if (rotaAtraso) await page.route(rotaAtraso, async r => { await new Promise(x => setTimeout(x, 5000)); r.continue(); });
  await page.goto(`${ORIG}/index.html?screen=dashboard`);
  await page.waitForTimeout(rotaAtraso ? 400 : 1200);
  out[nome] = { erros };
  for (const t of telas) {
    await page.evaluate(x => { location.hash = x; }, t);
    await page.waitForTimeout(rotaAtraso ? 200 : 700);
    if (depois) await depois(page, t);
    out[nome][t] = await ler(page);
    await page.screenshot({ path: join(CAP, `estado_${nome}_${t}.png`) });
  }
  await ctx.close();
}
const reset = () => Object.assign(modo, { tarefas: 'normal', config: 'normal', semjson: false, semtextos: false, atraso: 0 });

await rodada('carregando', { rotaAtraso: /\/(assets\/(equipe|ferramentas|absorcao|config-catalogo)\.json|v1\/.*)$/ });
reset(); modo.tarefas = 'vazio'; await rodada('vazio', { telas: ['tarefas'] });
reset(); await rodada('filtro_sem_resultado', { telas: ['agentes', 'ferramentas', 'absorcao', 'config'], depois: async (p, t) => {
  const id = { agentes: '#agentesFiltro', ferramentas: '#ferramentasFiltro', absorcao: '#absorcaoFiltro', config: '#configBusca' }[t];
  await p.fill(id, 'zzzqqq-nada'); await p.waitForTimeout(500);
} });
reset(); modo.tarefas = 'erro'; modo.config = 'erro'; await rodada('erro_api', { telas: ['tarefas', 'config'] });
reset(); modo.tarefas = '401'; await rodada('token_recusado', { telas: ['tarefas'] });
reset(); modo.semjson = true; await rodada('sem_json', {});
reset(); modo.semjson = true; await rodada('sem_json_en', { idioma: 'en', telas: ['geral', 'agentes'] });
reset(); modo.semtextos = true; await rodada('sem_textos', { idioma: 'en', telas: ['geral', 'tarefas'] });
reset(); await rodada('sem_host', { tauri: false, telas: ['geral', 'ide'] });
reset();

// PWA sem rede: primeira visita com rede (o service worker instala a casca), depois sem rede.
{
  const ctx = await browser.newContext({ viewport: { width: 390, height: 844 }, locale: 'pt-BR', isMobile: true, hasTouch: true });
  await ctx.addInitScript(() => { try { localStorage.setItem('phxclaw.token', 'token-de-teste'); } catch {} });
  const page = await ctx.newPage();
  await page.goto(`${ORIG}/?tela=tarefas`);
  await page.waitForFunction(() => navigator.serviceWorker?.controller || navigator.serviceWorker?.ready.then(() => true), null, { timeout: 15000 }).catch(() => {});
  await page.waitForTimeout(2500);
  const cache = await page.evaluate(async () => { const ks = await caches.keys(); const r = {}; for (const k of ks) r[k] = (await (await caches.open(k)).keys()).map(q => new URL(q.url).pathname); return r; });
  await ctx.setOffline(true);
  const falhas = [];
  page.on('requestfailed', r => falhas.push(new URL(r.url()).pathname + ' ' + (r.failure()?.errorText || '')));
  page.on('response', r => { if (r.status() >= 400) falhas.push(new URL(r.url()).pathname + ' HTTP ' + r.status()); });
  await page.reload();
  await page.waitForTimeout(4500);
  out.offline = { cache, falhas: [] };
  for (const t of TELAS) {
    await page.evaluate(x => { location.hash = x; }, t);
    await page.waitForTimeout(800);
    out.offline[t] = await ler(page);
    await page.screenshot({ path: join(CAP, `estado_offline_${t}.png`) });
  }
  out.offline.fonte = await page.evaluate(async () => { await document.fonts.ready; return [...document.fonts].map(f => `${f.family} ${f.status}`); });
  out.offline.falhas = [...new Set(falhas)];
  await ctx.close();
}
writeFileSync(join(OUT, 'estados.json'), JSON.stringify(out, null, 1));
console.log(JSON.stringify(out, null, 1));
await browser.close(); srv.close();
