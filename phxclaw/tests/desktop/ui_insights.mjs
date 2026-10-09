// A tela Insights EXERCITADA contra o agente REAL (`phxclaw servir`): execucoes de verdade
// (dois fluxos pela rota /v1/fluxos/rodar, um que passa e um que falha lendo arquivo que nao
// existe), a rota /v1/insights pelo fio (Bearer, periodo invalido, filtro por fluxo), e a tela
// -- os cartoes com o MESMO numero da rota, as barras por estado, a falha mais comum com o
// motivo como foi gravado (dado sem caixa-alta), a lista por fluxo, os filtros de periodo e de
// fluxo, nas larguras 1280 e 400, nos dois temas, com o contraste medido (>= 4,5:1).
//
// Uso: PLAYWRIGHT_BROWSERS_PATH=/opt/pw-browsers node tests/desktop/ui_insights.mjs [BINARIO] [--ui DIR]
// Resultado em tests/desktop/out/ui_insights.json; capturas ui_insights_{1280,400}_{escuro,claro}.png.
// RED medido: copia da UI sem o <script src="./assets/insights.js"> deixa os cartoes vazios; sem
// o `.merge(crate::insights::rotas())` a rota da 404.
import { createRequire } from 'node:module';
import { spawn } from 'node:child_process';
import { mkdtempSync, writeFileSync, mkdirSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { vigiarCsp } from './seguranca.mjs';

const require = createRequire(import.meta.url);
const { chromium } = require('/opt/node22/lib/node_modules/playwright');
const AQUI = dirname(fileURLToPath(import.meta.url));
const RAIZ = resolve(AQUI, '../..');
const iUi = process.argv.indexOf('--ui');
const UI_DIR = iUi > 0 ? resolve(process.argv[iUi + 1]) : null;
const BIN = resolve((process.argv[2] && process.argv[2] !== '--ui' ? process.argv[2] : null) || join(RAIZ, 'target/debug/phxclaw'));
const OUT = join(AQUI, 'out');
mkdirSync(OUT, { recursive: true });
const checagens = [];
function check(nome, ok, detalhe = '') {
  checagens.push({ nome, ok: !!ok, detalhe: String(detalhe).slice(0, 400) });
  console.log(`${ok ? 'ok   ' : 'FALHA '}${nome}${detalhe ? ` :: ${String(detalhe).slice(0, 400)}` : ''}`);
}
const esperar = ms => new Promise(r => setTimeout(r, ms));

const D = mkdtempSync(join(tmpdir(), 'phx-ui-insights-'));
const PASTA = join(D, 'agente');
const FLUXOS = join(PASTA, 'fluxos');
mkdirSync(FLUXOS, { recursive: true });
// «Blumenau» e dado: o nome do fluxo aparece como gravado, nunca «BLUMENAU».
writeFileSync(join(FLUXOS, 'vendas.json'), JSON.stringify({ nome: 'vendas Blumenau', passos: [{ id: 'grava', ferramenta: 'write_file', args: { path: 'v.txt', content: 'ok' } }] }));
writeFileSync(join(FLUXOS, 'quebra.json'), JSON.stringify({ nome: 'quebra', passos: [{ id: 'le', ferramenta: 'read_file', args: { path: 'nao-existe.txt' } }] }));
const token = 'token-ui-insights-' + Date.now();
writeFileSync(join(PASTA, 'api.token'), token);
const porta = 18700 + Math.floor(Math.random() * 900);
const log = { agente: '' };
const agente = spawn(BIN, ['servir', '--porta', String(porta), '--pasta', PASTA], {
  env: { PATH: process.env.PATH, HOME: D, PHXCLAW_PROJETO: D, ...(UI_DIR ? { PHXCLAW_UI_DIR: UI_DIR } : {}), PHXCLAW_MODELO: 'ollama:falso', OLLAMA_HOST: 'http://127.0.0.1:1', PHXCLAW_API_TOKEN: token },
  stdio: ['ignore', 'pipe', 'pipe'],
});
agente.stdout.on('data', b => { log.agente += b; });
agente.stderr.on('data', b => { log.agente += b; });
const capturas = [];
const medidas = {};
let falhou = null;
const ORIG = `http://127.0.0.1:${porta}`;
const fio = (metodo, caminho, corpo, t = token) => fetch(`${ORIG}/v1/${caminho}`, {
  method: metodo,
  headers: { ...(t ? { Authorization: `Bearer ${t}` } : {}), 'Content-Type': 'application/json' },
  body: corpo === undefined ? undefined : JSON.stringify(corpo),
});

const CONTRASTE = `(() => {
  const rgb = s => { const m = String(s).match(/[\\d.]+/g) || [0, 0, 0]; return m.slice(0, 3).map(Number); };
  const lum = c => { const [r, g, b] = c.map(v => { v /= 255; return v <= 0.03928 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4; }); return 0.2126 * r + 0.7152 * g + 0.0722 * b; };
  const razao = (a, b) => { const [x, y] = [lum(rgb(a)), lum(rgb(b))].sort((p, q) => q - p); return +((x + 0.05) / (y + 0.05)).toFixed(2); };
  const fundo = e => { for (let n = e; n; n = n.parentElement) { const c = getComputedStyle(n).backgroundColor; if (c && !/rgba\\(.*, 0\\)$/.test(c) && c !== 'transparent') return c; } return getComputedStyle(document.body).backgroundColor; };
  const medir = [];
  for (const sel of ['#tela-insights h1', '#insightsStatus', '.insights-kpis .metric strong', '.insights-kpis .metric header span', '.insights-kpis .metric small', '.insights-painel h2', '.insights-rotulo', '.insights-n', '.insights-fluxos b', '.insights-fluxos span', '#insightsPeriodo', '#insightsAtualizar', '.insights-filtros label span']) {
    const e = document.querySelector(sel);
    if (e && e.offsetParent) medir.push({ nome: sel, razao: razao(getComputedStyle(e).color, fundo(e)) });
  }
  return medir;
})()`;

try {
  let vivo = false;
  for (let i = 0; i < 100 && !vivo; i++) {
    await esperar(200);
    try { vivo = (await fetch(`${ORIG}/health`)).ok; } catch {}
  }
  check('o agente real sobe e serve a UI', vivo, log.agente.split('\n').slice(-3).join(' | '));

  // ---------------------------------------------------------------- execucoes de verdade
  const ids = [];
  for (const nome of ['vendas.json', 'vendas.json', 'vendas.json', 'quebra.json', 'quebra.json']) {
    const r = await fio('POST', 'fluxos/rodar', { nome });
    ids.push((await r.json()).id);
  }
  const fim = async id => {
    for (let i = 0; i < 100; i++) {
      const t = await (await fio('GET', `tasks/${id}`)).json();
      if (['completed', 'failed'].includes(t.status)) return t;
      await esperar(100);
    }
    return null;
  };
  const finais = await Promise.all(ids.map(fim));
  check('cinco execucoes reais terminaram (3 ok, 2 falhas)', finais.filter(t => t?.status === 'completed').length === 3 && finais.filter(t => t?.status === 'failed').length === 2, JSON.stringify(finais.map(t => t?.status)));

  // ---------------------------------------------------------------- a rota pelo fio
  const semToken = await fio('GET', 'insights', undefined, null);
  check('rota: sem Bearer e 401', semToken.status === 401, semToken.status);
  const ruim = await fio('GET', 'insights?periodo=ontem');
  check('rota: periodo invalido e 400', ruim.status === 400, ruim.status);
  const v = await (await fio('GET', 'insights?periodo=7d')).json();
  check('rota: 5 execucoes, 3 concluidas e 2 falhas, taxa 40%',
    v.total === 5 && v.por_estado?.completed === 3 && v.por_estado?.failed === 2 && Math.abs(v.taxa_falha - 0.4) < 1e-9, JSON.stringify({ ...v, fluxos: undefined }));
  check('rota: p50/p95 de 5 execucoes terminadas', v.duracao?.amostras === 5 && v.duracao.p50_ms !== null && v.duracao.p95_ms >= v.duracao.p50_ms, JSON.stringify(v.duracao));
  check('rota: a falha mais comum agrupa as duas, e por fluxo separa os dois', v.falhas_comuns?.[0]?.n === 2 && v.fluxos?.length === 2 && v.fluxos.find(f => f.nome === 'quebra')?.taxa_falha === 1, JSON.stringify({ falhas: v.falhas_comuns, fluxos: v.fluxos }));
  const soQuebra = await (await fio('GET', `insights?periodo=7d&fluxo=${encodeURIComponent('quebra')}`)).json();
  check('rota: filtro por fluxo', soQuebra.total === 2 && soQuebra.fluxos.length === 1, JSON.stringify({ total: soQuebra.total }));

  // ---------------------------------------------------------------- a tela
  const browser = await chromium.launch();
  const erros = [];
  const vigias = [];
  async function abrirTela(largura, temaEscolhido) {
    const ctx = await browser.newContext({ viewport: { width: largura, height: largura < 640 ? 860 : 900 }, locale: 'pt-BR', deviceScaleFactor: 1, hasTouch: largura < 640 });
    vigias.push(await vigiarCsp(ctx));
    await ctx.addInitScript(([t, tm]) => { try { localStorage.setItem('phxclaw.token', t); localStorage.setItem('phxclaw.tema', tm); } catch {} }, [token, temaEscolhido]);
    const page = await ctx.newPage();
    page.on('pageerror', e => erros.push(`${largura}/${temaEscolhido}: ${e}`));
    await page.goto(`${ORIG}/?tela=insights`);
    await page.waitForSelector('#tela-insights:not([hidden])');
    await page.waitForSelector('.insights-kpis .metric strong', { timeout: 10000 });
    return { ctx, page };
  }
  const ler = page => page.evaluate(() => ({
    kpis: [...document.querySelectorAll('.insights-kpis .metric')].map(m => ({ rotulo: m.querySelector('header span').textContent, valor: m.querySelector('strong').textContent })),
    estados: [...document.querySelectorAll('.insights-barras.estados li')].map(l => l.textContent),
    falhas: [...document.querySelectorAll('.insights-barras.falhas li')].map(l => ({ t: l.querySelector('.insights-rotulo')?.textContent, n: l.querySelector('.insights-n')?.textContent, tt: getComputedStyle(l.querySelector('.insights-rotulo') || l).textTransform })),
    fluxos: [...document.querySelectorAll('.insights-fluxos li')].map(l => ({ nome: l.querySelector('b')?.textContent, tt: l.querySelector('b') ? getComputedStyle(l.querySelector('b')).textTransform : '', linha: l.querySelector('span')?.textContent })),
    opcoes: [...document.querySelectorAll('#insightsFluxo option')].map(o => o.value),
    status: document.getElementById('insightsStatus').textContent,
    fonte: document.getElementById('insightsConteudo').dataset.fonte,
  }));

  for (const [largura, tm] of [[1280, 'escuro'], [1280, 'claro'], [400, 'escuro'], [400, 'claro']]) {
    const { ctx, page } = await abrirTela(largura, tm);
    const t = await ler(page);
    if (largura === 1280 && tm === 'escuro') {
      check('tela: o cartao EXECUÇÕES mostra o numero da rota', t.kpis[0]?.rotulo === 'EXECUÇÕES' && t.kpis[0]?.valor === String(v.total), JSON.stringify(t.kpis));
      check('tela: a taxa de falha da rota (40%)', /^40(,0)?%$/.test(t.kpis[1]?.valor || ''), JSON.stringify(t.kpis[1]));
      check('tela: por estado traduzido pelo formatador da tela Tarefas', t.estados.some(e => /CONCLUÍDA.*3/.test(e)) && t.estados.some(e => /FALHOU.*2/.test(e)), JSON.stringify(t.estados));
      const motivo = v.falhas_comuns[0].motivo;
      check('tela: a falha mais comum com o motivo COMO GRAVADO (dado, sem caixa-alta)', t.falhas[0]?.t === motivo && t.falhas[0]?.n === '2' && t.falhas[0]?.tt === 'none', JSON.stringify(t.falhas));
      const vendas = t.fluxos.find(f => f.nome === 'vendas Blumenau');
      check('tela: o fluxo pelo nome gravado («vendas Blumenau», nunca «VENDAS BLUMENAU») e a linha dele', vendas && vendas.tt === 'none' && /3 execuções · 0 falhas/.test(vendas.linha), JSON.stringify(t.fluxos));
      check('tela: o filtro de fluxo lista os dois que rodaram', t.opcoes.includes('quebra') && t.opcoes.includes('vendas Blumenau') && t.opcoes[0] === '', JSON.stringify(t.opcoes));
      check('tela: a fonte do numero esta declarada (data-fonte)', t.fonte === 'api /v1/insights', t.fonte);
      // Filtro de fluxo: a tela pede a rota de novo e o cartao muda.
      await page.selectOption('#insightsFluxo', 'quebra');
      await page.waitForFunction(() => document.querySelector('.insights-kpis .metric strong')?.textContent === '2', null, { timeout: 5000 }).catch(() => {});
      const q = await ler(page);
      check('tela: filtrar por «quebra» mostra 2 execucoes e 100% de falha', q.kpis[0]?.valor === '2' && /^100(,0)?%$/.test(q.kpis[1]?.valor || '') && q.opcoes.includes('vendas Blumenau'), JSON.stringify(q.kpis));
      await page.selectOption('#insightsFluxo', '');
      await page.selectOption('#insightsPeriodo', '24h');
      await page.waitForFunction(() => document.querySelector('.insights-kpis .metric strong')?.textContent === '5', null, { timeout: 5000 }).catch(() => {});
      const p = await ler(page);
      check('tela: trocar o periodo pede a rota de novo (24h: as mesmas 5)', p.kpis[0]?.valor === '5' && /5 execuções/.test(p.status), JSON.stringify({ k: p.kpis[0], s: p.status }));
      // Idioma: a mesma tela em ingles pela fabrica; o dado continua como gravado.
      await page.evaluate(() => idiomas.trocar('en'));
      await page.waitForTimeout(400);
      const en = await ler(page);
      check('tela: em ingles os rotulos mudam e o dado nao', en.kpis[0]?.rotulo === 'EXECUTIONS' && en.fluxos.some(f => f.nome === 'vendas Blumenau'), JSON.stringify({ k: en.kpis[0], f: en.fluxos.map(f => f.nome) }));
      await page.evaluate(() => idiomas.trocar('pt'));
      await page.waitForTimeout(300);
      const pt = await ler(page);
      check('tela: de volta ao portugues, o resumo tambem volta (nao fica em ingles)', /execuções de/.test(pt.status) && !/executions/.test(pt.status), pt.status);
    }
    const m = await page.evaluate(() => ({
      tema: document.documentElement.dataset.tema,
      paginaLarga: document.documentElement.scrollWidth > innerWidth + 1,
      telaLarga: document.getElementById('tela-insights').scrollWidth > document.getElementById('tela-insights').clientWidth + 1,
      colunas: getComputedStyle(document.querySelector('.insights-grade')).gridTemplateColumns.split(' ').length,
    }));
    check(`${largura}/${tm}: o tema pedido esta aplicado`, m.tema === tm, m.tema);
    check(`${largura}/${tm}: nada passa da largura (${largura === 400 ? 'uma coluna' : 'duas colunas'})`, !m.paginaLarga && !m.telaLarga && m.colunas === (largura === 400 ? 1 : 2), JSON.stringify(m));
    const c = await page.evaluate(CONTRASTE);
    medidas[`contraste_${largura}_${tm}`] = c;
    const ruins = c.filter(x => x.razao < 4.5);
    check(`${largura}/${tm}: contraste >= 4,5:1 em todo texto medido (${c.length} amostras)`, c.length >= 10 && !ruins.length, JSON.stringify(ruins.length ? ruins : c.map(x => `${x.nome}=${x.razao}`)));
    const nome = `ui_insights_${largura}_${tm}.png`;
    await page.screenshot({ path: join(OUT, nome), fullPage: false });
    capturas.push(nome);
    await ctx.close();
  }
  check('nenhum erro de pagina', !erros.length, erros.join(' | '));
  const csp = vigias.flatMap(x => x.vistas);
  check('zero violacao de CSP (o agente real manda a da tela)', csp.length === 0, csp.slice(0, 3).join(' | '));
  await browser.close();
} catch (e) {
  falhou = e;
  check('o roteiro terminou sem excecao', false, e.stack || e);
} finally {
  agente.kill('SIGTERM');
  rmSync(D, { recursive: true, force: true });
}
const passou = checagens.filter(c => c.ok).length;
console.log(`placar: ${passou}/${checagens.length}`);
writeFileSync(join(OUT, 'ui_insights.json'), JSON.stringify({ roteiro: 'tests/desktop/ui_insights.mjs', medido_em: new Date().toISOString(), placar: `${passou}/${checagens.length}`, ok: passou === checagens.length && checagens.length > 0 && !falhou, checagens, medidas, capturas }, null, 1));
process.exit(checagens.length && passou === checagens.length && !falhou ? 0 : 1);
