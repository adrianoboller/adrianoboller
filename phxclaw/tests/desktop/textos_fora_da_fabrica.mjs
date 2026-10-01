// Conta os textos de tela que ainda NAO passam pela fabrica de idiomas (assets/textos.json),
// medindo o que a pessoa ve -- e nao o fonte. Pseudo-localizacao no Chromium:
//
//   * os JSON de DADO (equipe, ferramentas, absorcao) sao servidos com toda string trocada
//     por "§" -- rotulo se traduz, dado nunca, entao dado some da conta;
//   * o textos.json e servido com todo valor trocado por "⟦chave⟧" -- o que a fabrica
//     resolveu some da conta;
//   * sobra o que alguem escreveu cru no HTML ou no JS: e isso que se conta.
//
// Percorre varios estados (com e sem host Tauri, host com erro, arquivo ausente, splash
// em andamento, terminal aberto), porque texto que so aparece num estado tambem e texto.
// E confere o laco nos dois sentidos: chave pedida pela tela que nao existe na fabrica, e
// chave da fabrica que ninguem pede (chave morta e pior que chave faltando).
//
// Uso: PLAYWRIGHT_BROWSERS_PATH=/opt/pw-browsers node tests/desktop/textos_fora_da_fabrica.mjs [--tudo] [--isentos] [--ui DIR]
// Sai 1 se o placar passar do TETO (catraca: so desce) ou se o laco quebrar.
import { createRequire } from 'node:module';
import { readFileSync, existsSync } from 'node:fs';
import { dirname, join, extname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

// Catraca: SO DESCE. Traduziu N textos, baixe o teto no mesmo commit.
// 140 medidos antes da fabrica (mesma arvore, sem textos.json); 119 com o primeiro lote
// (menu lateral + Agentes + IDE + aviso de arquivo ausente); 46 com Visao geral,
// Ferramentas e Absorcao. Falta: barra do topo, splash, rodape e o painel do host.
const TETO = 46;

// Texto que nao se traduz, com o motivo: nome proprio, marca, sigla tecnica, identificador.
// Comparado depois de tirar dado e chave, sem espaco nas pontas.
const ISENTOS = new Map([
  ['PhxClaw • Command Center', 'marca (titulo da janela)'], ['PhxClaw', 'marca'],
  ['PHOENIX', 'marca'], ['CLAW', 'marca'], ['PO', 'iniciais do avatar'],
  ['PT', 'codigo do idioma no botao de troca'], ['EN', 'codigo do idioma no botao de troca'],
  ['ZERO TRUST', 'nome da politica de seguranca (config/constitution.json)'],
  ['Deny-by-default', 'nome da politica de seguranca (config/constitution.json)'],
  ['IDE', 'sigla (nome da tela)'], ['bash —', 'nome do programa na aba; o resto e o titulo do terminal (dado)'],
  ['OpenClaw', 'produto'], ['Hermes', 'produto'], ['Claude Code', 'produto'], ['Codex', 'produto'], ['OpenJarvis', 'produto'],
  // Etapas do boot: nome de modulo do kernel, o mesmo nome do crate e do log.
  ['Microkernel', 'modulo'], ['Research Core', 'modulo'], ['Hypothesis Core', 'modulo'], ['Installer Core', 'modulo'],
  ['Plugin Registry', 'modulo'], ['Agent Runtime', 'modulo'], ['Task Graph', 'modulo'],
  ['Model Gateway', 'modulo'], ['Event Bus', 'modulo'], ['Connectivity', 'modulo'], ['Desktop Host', 'modulo'],
  ['Evidence Ledger', 'modulo'], ['Desktop Fabric', 'modulo'], ['Media & Documents', 'modulo'], ['Mindset + BPM', 'modulo'],
]);

const require = createRequire(import.meta.url);
const { chromium } = require('/opt/node22/lib/node_modules/playwright');
const AQUI = dirname(fileURLToPath(import.meta.url));
const RAIZ = resolve(AQUI, '../..');
const iUi = process.argv.indexOf('--ui');
const UI = iUi > 0 ? resolve(process.argv[iUi + 1]) : join(RAIZ, 'apps/phxclaw-ui');
const ORIGEM = 'http://phxclaw.local';
const TIPOS = { '.html': 'text/html', '.js': 'text/javascript', '.css': 'text/css', '.svg': 'image/svg+xml', '.json': 'application/json', '.woff2': 'font/woff2' };
const TUDO = process.argv.includes('--tudo');
const DADOS = new Set(['equipe.json', 'ferramentas.json', 'absorcao.json']);

const apagarStrings = v => Array.isArray(v) ? v.map(apagarStrings)
  : v && typeof v === 'object' ? Object.fromEntries(Object.entries(v).map(([k, x]) => [k, apagarStrings(x)]))
  : typeof v === 'string' ? '§' : v;

const caminhoTextos = join(UI, 'assets/textos.json');
const fabrica = existsSync(caminhoTextos) ? JSON.parse(readFileSync(caminhoTextos, 'utf8')) : null;
function pseudoFabrica() {
  if (!fabrica) return null;
  const textos = {};
  for (const k of Object.keys(fabrica.textos)) {
    textos[k] = {};
    for (const i of fabrica.idiomas) textos[k][i] = `⟦${k}⟧`;
  }
  return { ...fabrica, textos };
}

function stubTauri(modo) {
  if (modo === 'sem-host') return;
  const ouvintes = {};
  window.__chamadas = [];
  window.__emitir = (ev, payload) => (ouvintes[ev] || []).forEach(f => f({ payload }));
  window.__TAURI__ = {
    event: { listen: async (ev, f) => { (ouvintes[ev] ||= []).push(f); return () => {}; } },
    core: {
      invoke: async (cmd, args) => {
        window.__chamadas.push({ cmd, args });
        if (modo === 'host-erro' && cmd === 'host_status') throw new Error('falha simulada');
        switch (cmd) {
          case 'host_status': return { version: '0', session_uuid: '0', policy: {}, live_bus: { receiver_count: 1 }, api: null };
          case 'verify_evidence': return { valid: modo !== 'host-invalido', records: 1 };
          case 'events_snapshot': return [];
          case 'terminal_abrir': return { id: 't1', pid: 1, programa: args.programa, cwd: '/' };
          default: return null;
        }
      },
    },
  };
}

const coletar = page => page.evaluate(() => {
  const achados = [];
  const andar = n => {
    if (n.nodeType === 3) { achados.push(n.textContent); return; }
    if (n.nodeType !== 1 || ['SCRIPT', 'STYLE'].includes(n.tagName)) return;
    for (const a of ['placeholder', 'aria-label', 'title', 'alt']) if (n.hasAttribute(a)) achados.push(n.getAttribute(a));
    n.childNodes.forEach(andar);
  };
  andar(document.body);
  achados.push(document.title);
  return achados;
});

let trocaOk = false;
let trocaDetalhe = '';
const cravados = new Map();   // texto -> estados onde apareceu
const isentosVistos = new Set();
function registrar(estado, lista) {
  for (const bruto of lista) {
    // A hora do console de eventos e formatada pelo navegador (toLocaleTimeString): dado.
    const t = bruto.replace(/⟦[^⟧]*⟧/g, ' ').replace(/§/g, ' ').replace(/\d+:\d+:\d+( [AP]M)?/g, ' ')
      .replace(/\s+/g, ' ').trim();
    if (!/\p{L}\p{L}/u.test(t) && !/^\p{Lu}$/u.test(t)) continue;
    if (ISENTOS.has(t)) { isentosVistos.add(t); continue; }
    if (!cravados.has(t)) cravados.set(t, new Set());
    cravados.get(t).add(estado);
  }
}

async function abrir(browser, { modo = 'host', semEquipe = false, url = '?screen=dashboard', real = false } = {}) {
  const page = await browser.newPage({ viewport: { width: 1560, height: 960 } });
  await page.route(`${ORIGEM}/**`, route => {
    const caminho = decodeURIComponent(new URL(route.request().url()).pathname);
    const arq = join(UI, caminho === '/' ? 'index.html' : caminho);
    const nome = caminho.split('/').pop();
    if ((semEquipe && nome === 'equipe.json') || !arq.startsWith(UI) || !existsSync(arq)) return route.fulfill({ status: 404, body: 'nao existe' });
    if (!real && DADOS.has(nome)) return route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify(apagarStrings(JSON.parse(readFileSync(arq, 'utf8')))) });
    if (!real && nome === 'textos.json') return route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify(pseudoFabrica()) });
    return route.fulfill({ status: 200, body: readFileSync(arq), contentType: TIPOS[extname(arq)] || 'application/octet-stream' });
  });
  await page.addInitScript(stubTauri, modo);
  await page.goto(`${ORIGEM}/index.html${url}`);
  // So mede depois de a fabrica chegar: antes dela a tela mostra o padrao do fonte, e a
  // conta oscilaria com o tempo de rede (oscilou: 121 e 140 na mesma arvore).
  await page.evaluate(() => (typeof idiomas !== 'undefined' ? idiomas.pronto : null));
  await page.waitForTimeout(300);
  return page;
}

async function percorrer(page, estado) {
  for (const tela of ['geral', 'agentes', 'ide', 'ferramentas', 'absorcao']) {
    await page.click(`.nav[data-tela="${tela}"]`);
    await page.waitForTimeout(250);
    registrar(estado, await coletar(page));
  }
}

const browser = await chromium.launch();
try {
  let p = await abrir(browser);
  await page_evento(p);
  await percorrer(p, 'host');
  await p.click('.nav[data-tela="agentes"]');
  await p.fill('#agentesFiltro', 'x');
  await p.waitForTimeout(150);
  registrar('host+filtro', await coletar(p));
  await p.click('.nav[data-tela="ide"]');
  await p.click('#ideAbrirBash');
  await p.waitForTimeout(150);
  await p.evaluate(() => window.__emitir('terminal_grade', { id: 't1', completa: true, colunas: 10, linhas: 2, fundo: 0, frente: 0, linhas_alteradas: [], cursor: null, titulo: '§', encerrado: { codigo: 0 } }));
  await p.waitForTimeout(150);
  registrar('host+terminal', await coletar(p));
  await p.close();

  p = await abrir(browser, { modo: 'sem-host' });
  await percorrer(p, 'sem-host');
  await p.click('.nav[data-tela="ide"]');
  await p.click('#ideAbrirBash');
  registrar('sem-host+terminal', await coletar(p));
  await p.close();

  for (const modo of ['host-erro', 'host-invalido']) {
    p = await abrir(browser, { modo });
    registrar(modo, await coletar(p));
    await p.close();
  }

  p = await abrir(browser, { semEquipe: true });
  await percorrer(p, 'sem-equipe');
  await p.close();

  // Splash: o rotulo do boot troca a cada 190 ms; amostra durante a animacao inteira.
  p = await abrir(browser, { url: '?screen=splash' });
  for (let i = 0; i < 80; i++) {
    registrar('splash', await coletar(p));
    await p.waitForTimeout(50);
  }
  await p.close();

  // A troca de idioma de verdade, com a fabrica real: abre em ingles pela URL, confere um
  // rotulo do HTML e um do JS, e o botao volta ao portugues sem recarregar.
  p = await abrir(browser, { url: '?screen=dashboard&idioma=en', real: true });
  await p.click('.nav[data-tela="agentes"]');
  await p.waitForTimeout(300);
  const lerTroca = () => p.evaluate(() => ({
    menu: document.querySelector('.nav[data-tela="agentes"] em').textContent,
    marca: document.querySelector('#agentesConteudo .marcas span')?.textContent,
    ph: document.getElementById('agentesFiltro').placeholder,
    botao: document.getElementById('trocarIdioma').textContent,
    lang: document.documentElement.lang,
  }));
  const en = await lerTroca();
  await p.click('#trocarIdioma');
  await p.waitForTimeout(300);
  const pt = await lerTroca();
  trocaOk = en.menu === fabrica?.textos['menu.agentes']?.en && [fabrica?.textos['agentes.agente']?.en, fabrica?.textos['agentes.humano']?.en].includes(en.marca)
    && en.ph === fabrica?.textos['agentes.filtro']?.en && en.botao === 'EN'
    && pt.menu === 'Agentes' && ['AGENTE', 'HUMANO'].includes(pt.marca) && pt.botao === 'PT' && pt.lang === 'pt-BR';
  trocaDetalhe = JSON.stringify({ en, pt });
  await p.close();
} finally {
  await browser.close();
}

async function page_evento(p) {
  await p.evaluate(() => window.__emitir?.('phoenix:event', { topic: '§', event_type: '§', uuid: '§', occurred_at: 0 }));
}

// Laco nos dois sentidos, lido do fonte: o que a tela pede e o que a fabrica tem.
const html = readFileSync(join(UI, 'index.html'), 'utf8');
const js = readFileSync(join(UI, 'assets/app.js'), 'utf8');
const pedidas = new Set([
  ...[...html.matchAll(/data-txt(?:-ph|-al|-tt)?="([^"]+)"/g)].map(m => m[1]),
  ...[...js.matchAll(/\btxt\(\s*'([^']+)'/g)].map(m => m[1]),
]);
const existentes = new Set(fabrica ? Object.keys(fabrica.textos) : []);
const faltando = [...pedidas].filter(k => !existentes.has(k));
const mortas = [...existentes].filter(k => !pedidas.has(k));
const ptVazio = fabrica ? Object.entries(fabrica.textos).filter(([, v]) => !v.pt).map(([k]) => k) : [];

const lista = [...cravados.keys()].sort();
if (TUDO) for (const t of lista) console.log(`  ${JSON.stringify(t)}  [${[...cravados.get(t)].join(',')}]`);
if (process.argv.includes('--isentos')) for (const t of [...isentosVistos].sort()) console.log(`  isento ${JSON.stringify(t)} :: ${ISENTOS.get(t)}`);
const porIdioma = fabrica ? fabrica.idiomas.map(i => `${i} ${Object.values(fabrica.textos).filter(v => v[i]).length}`).join(' • ') : 'sem textos.json';
console.log(`textos cravados (fora da fabrica): ${lista.length}  teto ${TETO}`);
console.log(`isentos vistos: ${isentosVistos.size}  isentos sem uso: ${ISENTOS.size - isentosVistos.size}`);
console.log(`fabrica: ${existentes.size} chaves • pedidas pela tela ${pedidas.size} • ${porIdioma}`);
console.log(`laco: faltando ${faltando.length}${faltando.length ? ` (${faltando.join(', ')})` : ''} • mortas ${mortas.length}${mortas.length ? ` (${mortas.join(', ')})` : ''} • pt vazio ${ptVazio.length}`);
console.log(`troca de idioma (en -> botao -> pt): ${trocaOk ? 'ok' : `FALHA ${trocaDetalhe}`}`);
const ok = trocaOk && lista.length <= TETO && !faltando.length && !mortas.length && !ptVazio.length;
process.exit(ok ? 0 : 1);
