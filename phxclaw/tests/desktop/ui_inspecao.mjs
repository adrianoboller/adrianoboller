// Prova do bloqueio do inspetor (`ui.bloquear_inspecao`) e da CSP da tela, EXERCITANDO no
// Chromium com teclado e mouse de verdade (page.keyboard / clique direito: eventos isTrusted).
//
// Duas fases:
//   1. UI do disco com os cabecalhos que saem do pwa.rs (seguranca.mjs) e a politica falsa:
//      ligada, desligada, sem resposta (404) e no desktop (stub do Tauri com a politica
//      desligada -- no desktop vale sempre).
//   2. O agente REAL (`phxclaw servir`): a rota /ui/politica pelo leitor do config, com o
//      config.json da pasta dizendo false, e os cabecalhos que o servidor manda de fato.
//
// O que se confere em cada caso: F12, Ctrl+Shift+I/J/C, Ctrl+U e Cmd+Option+I tem a acao
// padrao cancelada (defaultPrevented) quando ligado e intacta quando desligado; o clique
// direito fora de campo idem; DENTRO de campo de texto o menu fica (colar e edicao); Tab move
// o foco e Ctrl+C nao e tocado em nenhum caso; e zero violacao de CSP no console.
//
// O que NAO se prova aqui: que o DevTools do navegador deixou de abrir -- o Chromium sem
// cabeca nao tem DevTools para abrir. O que se mede e o que a pagina pode fazer: cancelar a
// acao padrao. E e por isso que, no navegador, isto e dissuasao (docs/GUIA_DO_OPERADOR.md).
//
// Uso: PLAYWRIGHT_BROWSERS_PATH=/opt/pw-browsers node tests/desktop/ui_inspecao.mjs [BINARIO] [--ui DIR]
// Sai 0 so se todas as checagens passarem. RED medido: ver o fim do arquivo.
import { createRequire } from 'node:module';
import { readFileSync, existsSync, mkdtempSync, mkdirSync, writeFileSync, rmSync } from 'node:fs';
import { dirname, join, extname, resolve } from 'node:path';
import { tmpdir } from 'node:os';
import { spawn } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { cabecalhosDaUi, vigiarCsp } from './seguranca.mjs';

const require = createRequire(import.meta.url);
const { chromium } = require('/opt/node22/lib/node_modules/playwright');
const AQUI = dirname(fileURLToPath(import.meta.url));
const RAIZ = resolve(AQUI, '../..');
const iUi = process.argv.indexOf('--ui');
const UI = resolve(iUi > 0 ? process.argv[iUi + 1] : join(RAIZ, 'apps/phxclaw-ui'));
const BIN = resolve((process.argv[2] && process.argv[2] !== '--ui' ? process.argv[2] : null) || join(RAIZ, 'target/debug/phxclaw'));
const ORIGEM = 'http://phxclaw.local';
const TIPOS = { '.html': 'text/html', '.js': 'text/javascript', '.css': 'text/css', '.svg': 'image/svg+xml', '.json': 'application/json', '.woff2': 'font/woff2', '.png': 'image/png', '.webmanifest': 'application/manifest+json' };
const CABECALHOS = cabecalhosDaUi();

const checagens = [];
function check(nome, ok, detalhe = '') {
  checagens.push(!!ok);
  console.log(`${ok ? 'ok   ' : 'FALHA '}${nome}${detalhe ? ` :: ${detalhe}` : ''}`);
}

// Atalhos do inspetor e o que nao pode ser tocado. Modificadores no formato do Playwright.
const BLOQUEADOS = ['F12', 'Control+Shift+KeyI', 'Control+Shift+KeyJ', 'Control+Shift+KeyC', 'Control+KeyU', 'Meta+Alt+KeyI', 'Meta+Shift+KeyC'];
const LIVRES = ['Control+KeyC', 'Control+KeyA', 'Shift+Tab'];

// Registra, na bolha da janela (DEPOIS da captura do inspecao.js), se a acao padrao saiu
// cancelada. So evento confiavel conta: e o que o navegador usaria para abrir o inspetor.
const REGISTRO = () => {
  window.__vistos = [];
  const anota = e => {
    if (!e.isTrusted) return;
    const nome = e.type === 'contextmenu' ? `menu:${e.target.tagName}` : `${e.ctrlKey ? 'Control+' : ''}${e.metaKey ? 'Meta+' : ''}${e.altKey ? 'Alt+' : ''}${e.shiftKey ? 'Shift+' : ''}${e.code}`;
    window.__vistos.push({ nome, cancelado: e.defaultPrevented });
  };
  window.addEventListener('keydown', anota);
  window.addEventListener('contextmenu', anota);
};

async function exercitar(page, rotulo, esperaBloqueio) {
  await page.evaluate(REGISTRO);
  // Foco no corpo (fora de campo): tirar o foco, sem clicar em nada que navegue.
  const semFoco = () => page.evaluate(() => { document.activeElement && document.activeElement.blur && document.activeElement.blur(); });
  await semFoco();
  for (const t of BLOQUEADOS) await page.keyboard.press(t);
  for (const t of LIVRES) await page.keyboard.press(t);
  // Clique direito fora de campo e dentro de um campo de texto.
  await page.locator('#coreStatus').click({ button: 'right' });
  await page.locator('#tarefasObjetivo').click({ button: 'right' });
  const vistos = await page.evaluate(() => window.__vistos);
  const achar = n => vistos.filter(v => v.nome === n);
  for (const t of BLOQUEADOS) {
    const v = achar(t);
    check(`${rotulo}: ${t} ${esperaBloqueio ? 'bloqueado' : 'livre'}`, v.length === 1 && v[0].cancelado === esperaBloqueio, JSON.stringify(v));
  }
  for (const t of LIVRES) {
    const v = achar(t);
    check(`${rotulo}: ${t} intocado`, v.length === 1 && v[0].cancelado === false, JSON.stringify(v));
  }
  const menus = vistos.filter(v => v.nome.startsWith('menu:'));
  const fora = menus.find(v => !['INPUT', 'TEXTAREA'].includes(v.nome.slice(5)));
  const dentro = menus.find(v => ['INPUT', 'TEXTAREA'].includes(v.nome.slice(5)));
  check(`${rotulo}: clique direito fora de campo ${esperaBloqueio ? 'bloqueado' : 'livre'}`, fora && fora.cancelado === esperaBloqueio, JSON.stringify(fora));
  check(`${rotulo}: clique direito em campo de texto mantem o menu (colar)`, dentro && dentro.cancelado === false, JSON.stringify(dentro));
  // Tab continua movendo o foco (acessibilidade por teclado).
  await semFoco();
  const antes = await page.evaluate(() => document.activeElement && document.activeElement.outerHTML.slice(0, 60));
  await page.keyboard.press('Tab');
  await page.keyboard.press('Tab');
  const depois = await page.evaluate(() => document.activeElement && document.activeElement.outerHTML.slice(0, 60));
  check(`${rotulo}: Tab move o foco`, antes !== depois, `${antes} -> ${depois}`);
}

// ---------------------------------------------------------------- fase 1: UI do disco
async function faseDisco(browser) {
  const casos = [
    { rotulo: 'politica ligada', politica: { bloquear_inspecao: true }, espera: true },
    { rotulo: 'politica desligada', politica: { bloquear_inspecao: false }, espera: false },
    { rotulo: 'politica sem resposta (404)', politica: null, espera: true },
    { rotulo: 'desktop com politica desligada', politica: { bloquear_inspecao: false }, espera: true, tauri: true },
  ];
  for (const caso of casos) {
    const ctx = await browser.newContext({ viewport: { width: 1366, height: 800 }, locale: 'pt-BR' });
    if (caso.tauri) {
      await ctx.addInitScript(() => {
        window.__TAURI__ = { core: { invoke: async () => ({}) }, event: { listen: async () => () => {} } };
      });
    }
    const vigia = await vigiarCsp(ctx);
    const page = await ctx.newPage();
    const erros = [];
    page.on('pageerror', e => erros.push(String(e)));
    await page.route(`${ORIGEM}/**`, route => {
      const caminho = new URL(route.request().url()).pathname;
      if (caminho === '/ui/politica') {
        return caso.politica
          ? route.fulfill({ status: 200, contentType: 'application/json', headers: CABECALHOS, body: JSON.stringify(caso.politica) })
          : route.fulfill({ status: 404, headers: CABECALHOS, body: 'nao existe' });
      }
      if (caminho.startsWith('/v1/')) return route.fulfill({ status: 200, contentType: 'application/json', headers: CABECALHOS, body: '[]' });
      const arq = join(UI, caminho === '/' ? 'index.html' : caminho.slice(1));
      if (!arq.startsWith(UI) || !existsSync(arq)) return route.fulfill({ status: 404, headers: CABECALHOS, body: 'nao existe' });
      return route.fulfill({ status: 200, body: readFileSync(arq), contentType: TIPOS[extname(arq)] || 'application/octet-stream', headers: CABECALHOS });
    });
    await page.goto(`${ORIGEM}/index.html?tela=tarefas`);
    const existe = await page.waitForFunction(() => typeof inspecao !== 'undefined', null, { timeout: 5000 }).then(() => true, () => false);
    check(`${caso.rotulo}: o inspecao.js carregou`, existe);
    if (!existe) { await ctx.close(); continue; }
    await page.evaluate(() => inspecao.pronto);
    const estado = await page.evaluate(() => ({ ligado: inspecao.ligado(), marca: document.documentElement.dataset.inspecao }));
    check(`${caso.rotulo}: estado ${caso.espera ? 'ligado' : 'desligado'}`, estado.ligado === caso.espera, JSON.stringify(estado));
    await exercitar(page, caso.rotulo, caso.espera);
    check(`${caso.rotulo}: sem erro de pagina`, erros.length === 0, erros.join(' | '));
    const v = await vigia.todas(ctx.pages());
    check(`${caso.rotulo}: zero violacao de CSP`, v.length === 0, v.join(' | '));
    await ctx.close();
  }
}

// ---------------------------------------------------------------- fase 2: agente real
async function faseAgente(browser) {
  if (!existsSync(BIN)) { check(`binario ${BIN}`, false, 'nao existe: cargo build -p phxclaw'); return; }
  for (const [rotulo, config, espera] of [['agente padrao', null, true], ['agente com ui.bloquear_inspecao=false', { ui: { bloquear_inspecao: false } }, false]]) {
    const D = mkdtempSync(join(tmpdir(), 'phx-ui-inspecao-'));
    const PASTA = join(D, 'agente');
    mkdirSync(PASTA, { recursive: true });
    if (config) writeFileSync(join(PASTA, 'config.json'), JSON.stringify(config));
    const token = `token-ui-inspecao-${Date.now()}`;
    const porta = 18100 + Math.floor(Math.random() * 500);
    let log = '';
    const agente = spawn(BIN, ['servir', '--porta', String(porta), '--pasta', PASTA], {
      env: { PATH: process.env.PATH, HOME: D, PHXCLAW_PROJETO: D, PHXCLAW_UI_DIR: UI, PHXCLAW_MODELO: 'ollama:falso', OLLAMA_HOST: 'http://127.0.0.1:1', PHXCLAW_API_TOKEN: token },
      stdio: ['ignore', 'pipe', 'pipe'],
    });
    agente.stdout.on('data', b => { log += b; });
    agente.stderr.on('data', b => { log += b; });
    const ORIG = `http://127.0.0.1:${porta}`;
    try {
      let ok = false;
      for (let i = 0; i < 200 && !ok; i++) {
        try { ok = (await fetch(`${ORIG}/health`)).ok; } catch { await new Promise(r => setTimeout(r, 100)); }
      }
      check(`${rotulo}: subiu`, ok, log.slice(-400));
      if (!ok) continue;
      // Os cabecalhos que o servidor manda DE FATO, contra os do fonte.
      for (const rota of ['/', '/assets/app.js', '/ui/politica', '/v1/tasks']) {
        const r = await fetch(`${ORIG}${rota}`, { headers: rota.startsWith('/v1/') ? { Authorization: `Bearer ${token}` } : {} });
        const faltam = Object.entries(CABECALHOS).filter(([k, val]) => r.headers.get(k) !== val).map(([k]) => k);
        check(`${rotulo}: ${rota} com os cabecalhos de seguranca`, faltam.length === 0, faltam.join(', '));
      }
      const p = await (await fetch(`${ORIG}/ui/politica`)).json();
      check(`${rotulo}: /ui/politica diz ${espera}`, p.bloquear_inspecao === espera, JSON.stringify(p));
      const ctx = await browser.newContext({ viewport: { width: 1366, height: 800 }, locale: 'pt-BR' });
      const vigia = await vigiarCsp(ctx);
      const page = await ctx.newPage();
      await page.goto(`${ORIG}/?tela=tarefas`);
      await page.evaluate(() => inspecao.pronto);
      const ligado = await page.evaluate(() => inspecao.ligado());
      check(`${rotulo}: a tela ${espera ? 'bloqueia' : 'libera'}`, ligado === espera);
      await exercitar(page, rotulo, espera);
      const v = await vigia.todas(ctx.pages());
      check(`${rotulo}: zero violacao de CSP`, v.length === 0, v.join(' | '));
      await ctx.close();
    } finally {
      agente.kill();
      rmSync(D, { recursive: true, force: true });
    }
  }
}

const browser = await chromium.launch();
try {
  await faseDisco(browser);
  await faseAgente(browser);
} catch (e) {
  check('roteiro', false, String(e).slice(0, 300));
} finally {
  await browser.close();
}
const falhas = checagens.filter(c => !c).length;
console.log(`\n${checagens.length - falhas}/${checagens.length} checagens`);
process.exit(falhas ? 1 : 0);

// RED medido (09/10/2026), numa copia da UI (--ui):
//   * sem o <script src="./assets/inspecao.js"> no index.html: «o inspecao.js carregou» cai
//     nos quatro casos do disco (e a fase do agente para no mesmo ponto);
//   * inspecao.js sem o `if (editavel(e.target)) return;`: «clique direito em campo de texto
//     mantem o menu» cai nos casos ligados;
//   * a pergunta da politica sem o `noDesktop ||` (o desktop passa a obedecer o false):
//     «desktop com politica desligada» cai.
