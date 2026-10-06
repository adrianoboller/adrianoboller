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
// Sai 1 se o placar passar do TETO_SPLASH_POR_ETAPA (catraca: so desce) ou se o laco quebrar.
import { createRequire } from 'node:module';
import { readFileSync, existsSync } from 'node:fs';
import { dirname, join, extname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

// Catraca: SO DESCE. Traduziu N textos, baixe o teto no mesmo commit.
//
// TETO_SPLASH_POR_ETAPA substitui o antigo TETO (aposentado em 46, e nao subido): aquele
// media o splash amostrando pelo relogio de parede, e a coleta, mais lenta que uma etapa do
// boot, pulava rotulos -- a mesma arvore deu 33, 34 e 35. A regua nova para o relogio da
// pagina e coleta as 15 etapas, uma a uma; mede MAIS, entao nasce no numero medido do dia
// (35, ja com o painel do host traduzido) em vez de subir o teto velho. Historico da regua
// antiga: 140 antes da fabrica, 119 no primeiro lote, 46 com Visao geral/Ferramentas/Absorcao.
// Nesta regua: 35 -> 14 quando a abertura deixou de ser um temporizador de 15 etapas
// cravadas e passou a mostrar as etapas reais pela fabrica (qualificacao de 01/10/2026, lote 5).
// 14 -> 7 quando o topo passou a dizer o estado do host pela fabrica (lote 6); 7 -> 0 com o
// rodape pela fabrica e «COMMAND CENTER» isento como marca, igual ao titulo da janela (lote 9).
const TETO_SPLASH_POR_ETAPA = 0;

// Texto que nao se traduz, com o motivo: nome proprio, marca, sigla tecnica, identificador.
// Comparado depois de tirar dado e chave, sem espaco nas pontas.
const ISENTOS = new Map([
  ['PhxClaw • Command Center', 'marca (titulo da janela)'], ['PhxClaw', 'marca'],
  ['COMMAND CENTER', 'marca (o nome do produto, o mesmo do titulo da janela)'],
  ['Phx', 'marca'], ['Claw', 'marca'],
  // (So «PT» tem uso: a varredura roda em portugues, e o botao mostra o idioma de destino.
  // «EN» ficou isento sem uso ate 01/10/2026 e saiu: isento sem uso e porta aberta.)
  ['PT', 'codigo do idioma no botao de troca'],
  ['ZERO TRUST', 'nome da politica de seguranca (config/constitution.json)'],
  ['Deny-by-default', 'nome da politica de seguranca (config/constitution.json)'],
  ['IDE', 'sigla (nome da tela)'], ['bash —', 'nome do programa na aba; o resto e o titulo do terminal (dado)'],
  ['Helix —', 'nome do programa na aba (ide.js atualizarAbas); o resto e o titulo do terminal (dado)'],
  ['OpenClaw', 'produto'], ['Hermes', 'produto'], ['Claude Code', 'produto'], ['Codex', 'produto'], ['OpenJarvis', 'produto'], ['VS Code', 'produto'],
  // (As 15 etapas do boot, nomes de modulo do kernel, sairam com o temporizador da abertura:
  // isento sem uso e porta aberta para o texto voltar cravado sem ninguem ver.)
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
const DADOS = new Set(['equipe.json', 'ferramentas.json', 'absorcao.json', 'config-catalogo.json']);
// A vista do /v1/config que saiu do motor real (ui_config.mjs usa a mesma). Dado vira «§»;
// o que e protocolo (tipo, origem, natureza, as flags) fica, porque e ele que escolhe o
// editor e o rotulo -- e rotulo e o que esta regua conta.
const VISTA_CONFIG = JSON.parse(readFileSync(join(AQUI, 'dados/config_vista.json'), 'utf8'));
// As opcoes de um enum sao VALORES que se gravam no config.json (dado); o padrao e o valor
// tambem, de qualquer tipo -- «true» e o que esta no arquivo, nao um rotulo. So o valor de
// uma chave editavel fica com o tipo dele, porque e ele que marca o checkbox (sem texto).
const PROTOCOLO_CONFIG = new Set(['tipo', 'origem', 'natureza', 'editavel', 'segredo', 'segredo_presente']);
const dadoDeConfig = (c, k, v) => {
  if (v === null || PROTOCOLO_CONFIG.has(k)) return v;
  if (k === 'padrao' || (k === 'valor' && (!c.editavel || c.segredo))) return '§';
  if (k === 'valor' && typeof v === 'boolean') return v;
  return apagarStrings(v);
};
const vistaPseudo = () => ({
  revisao: VISTA_CONFIG.revisao,
  arquivos: { pasta: '§', projeto: null, projeto_ignorado: null },
  // A chave continua unica («§N»): e a identidade da linha, e o numero nao tem letra.
  chaves: VISTA_CONFIG.chaves.map((c, i) => ({
    ...Object.fromEntries(Object.entries(c).map(([k, v]) => [k, dadoDeConfig(c, k, v)])),
    chave: `§${i}`,
  })),
});

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
          case 'terminal_abrir': return { id: args.programa === 'helix' ? 't2' : 't1', pid: 1, programa: args.programa, cwd: '/' };
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

// A API de tarefas falsa: uma tarefa em cada estado, com plano, pergunta, passos, erro e
// artefato -- texto que so aparece com a tarefa num estado tambem e texto. Todo campo e
// DADO (vem do servidor), entao vai como "§".
const ESTADOS_DE_TAREFA = ['pending', 'awaiting_approval', 'awaiting_input', 'running', 'completed', 'failed', 'cancelled'];
const tarefaFalsa = (s, i) => ({
  id: `t${i}`, objective: '§', status: s, model: '§', created_at: `2026-10-01T00:00:0${i}Z`, updated_at: '§',
  plan: ['§'], steps: [{ n: 1, kind: '§', tool: null, outcome: 'ok', summary: '§' }, { n: 2, kind: '§', tool: '§', outcome: 'erro', summary: '§' }],
  answer: s === 'completed' ? '§' : null, error: s === 'failed' ? '§' : null,
  question: s === 'awaiting_input' ? '§' : null, artifacts: [{ path: '§' }],
});
async function armarApiDeTarefas(page, { putConfig = 200 } = {}) {
  await page.route(`${ORIGEM}/v1/**`, route => {
    const p = new URL(route.request().url()).pathname;
    if (p === '/v1/config') {
      const put = route.request().method() === 'PUT';
      const corpo = !put ? vistaPseudo() : putConfig === 409 ? { revisao_atual: 99 } : putConfig === 422 ? { erros: [{ chave: VISTA_CONFIG.chaves[0].chave, motivo: '§' }] } : { revisao: 4 };
      return route.fulfill({ status: put ? putConfig : 200, contentType: 'application/json', body: JSON.stringify(corpo) });
    }
    const m = p.match(/^\/v1\/tasks\/(t\d)$/);
    const corpo = m ? tarefaFalsa(ESTADOS_DE_TAREFA[+m[1].slice(1)], +m[1].slice(1))
      : p === '/v1/tasks' ? ESTADOS_DE_TAREFA.map(tarefaFalsa) : null;
    return corpo ? route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify(corpo) })
      : route.fulfill({ status: 401, contentType: 'application/json', body: '{"error":"§"}' });
  });
}

async function abrir(browser, { modo = 'host', semEquipe = false, url = '?screen=dashboard', real = false, relogio = false, api = false, putConfig = 200 } = {}) {
  const page = await browser.newPage({ viewport: { width: 1560, height: 960 } });
  await page.route(`${ORIGEM}/**`, route => {
    const caminho = decodeURIComponent(new URL(route.request().url()).pathname);
    const arq = join(UI, caminho === '/' ? 'index.html' : caminho);
    const nome = caminho.split('/').pop();
    if ((semEquipe && nome === 'equipe.json') || !arq.startsWith(UI) || !existsSync(arq)) return route.fulfill({ status: 404, body: 'nao existe' });
    if (!real && DADOS.has(nome)) {
      const d = apagarStrings(JSON.parse(readFileSync(arq, 'utf8')));
      // No catalogo do config o padrao e dado de qualquer tipo (true, 30...), como na vista.
      if (nome === 'config-catalogo.json') for (const c of d.chaves) if (c.padrao !== null) c.padrao = '§';
      return route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify(d) });
    }
    if (!real && nome === 'textos.json') return route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify(pseudoFabrica()) });
    return route.fulfill({ status: 200, body: readFileSync(arq), contentType: TIPOS[extname(arq)] || 'application/octet-stream' });
  });
  if (api) await armarApiDeTarefas(page, { putConfig });
  await page.addInitScript(stubTauri, modo);
  // Relogio parado: os timers da pagina so andam quando o roteiro manda (clock.runFor).
  if (relogio) { await page.clock.install({ time: 0 }); await page.clock.pauseAt(1000); }
  await page.goto(`${ORIGEM}/index.html${url}`);
  // So mede depois de a fabrica chegar: antes dela a tela mostra o padrao do fonte, e a
  // conta oscilaria com o tempo de rede (oscilou: 121 e 140 na mesma arvore).
  await page.evaluate(() => (typeof idiomas !== 'undefined' ? idiomas.pronto : null));
  await page.waitForTimeout(300);
  return page;
}

async function percorrer(page, estado) {
  for (const tela of ['geral', 'agentes', 'ide', 'ferramentas', 'absorcao', 'tarefas', 'config']) {
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
  await p.waitForTimeout(300);
  registrar('host+filtro', await coletar(p));
  // As grades (phx-grid): o texto do componente que so aparece num estado tambem e texto --
  // o menu de filtro aberto, o seletor de colunas aberto, a absorcao agrupada por estado e o
  // cubo. O que o phx-grid escreve cru tem de chegar pela fabrica (grades.js) ou contar aqui.
  await p.fill('#agentesFiltro', '');
  await p.waitForTimeout(300);
  await p.click('#agentesConteudo .phx-th[data-campo="nucleo"] .phx-fbtn');
  await p.waitForTimeout(200);
  registrar('grade-menu-filtro', await coletar(p));
  await p.keyboard.press('Escape');
  await p.click('#agentesConteudo .phx-colsel-btn', { force: true }).catch(() => {});
  await p.waitForTimeout(150);
  registrar('grade-colunas', await coletar(p));
  await p.click('.nav[data-tela="absorcao"]');
  await p.click('#absorcaoPorEstado');
  await p.waitForTimeout(200);
  registrar('grade-por-estado', await coletar(p));
  await p.click('#absorcaoVerCubo');
  await p.waitForTimeout(300);
  registrar('grade-cubo', await coletar(p));
  await p.click('.nav[data-tela="ide"]');
  await p.click('#ideAbrirBash');
  await p.waitForTimeout(150);
  await p.evaluate(() => window.__emitir('terminal_grade', { id: 't1', completa: true, colunas: 10, linhas: 2, fundo: 0, frente: 0, linhas_alteradas: [], cursor: null, titulo: '§', encerrado: { codigo: 0 } }));
  await p.waitForTimeout(150);
  registrar('host+terminal', await coletar(p));
  // A aba do Helix tem o nome do programa cravado no ide.js; sem abri-la aqui o texto nunca
  // aparecia a varredura (e o isento ficaria sem uso, porta aberta).
  await p.click('#ideAbrirHelix');
  await p.waitForTimeout(150);
  await p.evaluate(() => window.__emitir('terminal_grade', { id: 't2', completa: true, colunas: 10, linhas: 2, fundo: 0, frente: 0, linhas_alteradas: [], cursor: null, titulo: '§', encerrado: null }));
  await p.waitForTimeout(150);
  registrar('host+helix', await coletar(p));
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

  // Tarefas sem token, com token e a API respondendo, e cada tarefa aberta no detalhe.
  p = await abrir(browser, { api: true });
  await p.click('.nav[data-tela="tarefas"]');
  await p.waitForTimeout(200);
  registrar('tarefas-sem-token', await coletar(p));
  await p.fill('#tarefasTokenCampo', 'x'.repeat(24));
  await p.click('#tarefasToken button');
  await p.waitForTimeout(400);
  registrar('tarefas-lista', await coletar(p));
  for (let i = 0; i < ESTADOS_DE_TAREFA.length; i++) {
    await p.click(`#tarefasLista tbody tr[data-id="t${i}"]`);
    await p.waitForTimeout(250);
    registrar(`tarefa-${ESTADOS_DE_TAREFA[i]}`, await coletar(p));
  }
  await p.evaluate(() => localStorage.setItem('phxclaw.token', 'recusado'));
  await p.route(`${ORIGEM}/v1/**`, route => route.fulfill({ status: 401, contentType: 'application/json', body: '{"error":"§"}' }));
  await p.click('#tarefasNova button');
  await p.fill('#tarefasObjetivo', '§');
  await p.click('#tarefasNova button');
  await p.waitForTimeout(300);
  registrar('tarefas-recusado', await coletar(p));
  await p.route(`${ORIGEM}/v1/**`, route => route.fulfill({ status: 500, contentType: 'application/json', body: '{"error":"§"}' }));
  await p.click('#tarefasNova button');
  await p.fill('#tarefasObjetivo', '§');
  await p.click('#tarefasNova button');
  await p.waitForTimeout(300);
  registrar('tarefas-erro', await coletar(p));
  await p.route(`${ORIGEM}/v1/**`, route => route.fulfill({ status: 200, contentType: 'application/json', body: '[]' }));
  await p.evaluate(() => mostrarTela('tarefas'));
  await p.waitForTimeout(300);
  registrar('tarefas-vazia', await coletar(p));
  await p.close();

  // Configuracao com a API: carregada, com o diff aberto, e nas duas recusas do contrato.
  for (const putConfig of [409, 422]) {
    p = await abrir(browser, { api: true, putConfig });
    await p.evaluate(() => localStorage.setItem('phxclaw.token', 'x'.repeat(24)));
    await p.evaluate(() => mostrarTela('config'));
    await p.waitForTimeout(600);
    registrar('config', await coletar(p));
    // Edita um campo de texto com dado pseudo («§»): o diff mostra de/para sem nenhuma
    // letra de dado, e sobra so o rotulo -- que e o que se conta.
    await p.fill('#configConteudo input.cfg-ed[type="text"] >> nth=0', '§§');
    await p.press('#configConteudo input.cfg-ed[type="text"] >> nth=0', 'Tab');
    await p.waitForTimeout(300);
    await p.click('#configSalvar');
    await p.waitForTimeout(200);
    registrar('config-diff', await coletar(p));
    await p.click('#configConfirmar');
    await p.waitForTimeout(400);
    registrar(`config-${putConfig}`, await coletar(p));
    await p.close();
  }

  // Splash: o rotulo do boot troca a cada 190 ms. Amostrar pelo relogio de parede oscilava
  // (33, 34 e 35 na mesma arvore: a coleta demora mais que uma etapa e pula rotulos); com o
  // relogio da pagina parado, avanca-se uma etapa por vez e cada rotulo e coletado uma vez.
  p = await abrir(browser, { url: '?screen=splash', relogio: true });
  for (let i = 0; i < 24; i++) {
    registrar('splash', await coletar(p));
    await p.clock.runFor(200);
  }
  await p.close();

  // A troca de idioma de verdade, com a fabrica real: abre em ingles pela URL, confere um
  // rotulo do HTML e um do JS, e o botao volta ao portugues sem recarregar.
  p = await abrir(browser, { url: '?screen=dashboard&idioma=en', real: true });
  await p.click('.nav[data-tela="agentes"]');
  await p.waitForTimeout(300);
  const lerTroca = () => p.evaluate(() => ({
    menu: document.querySelector('.nav[data-tela="agentes"] em').textContent,
    marca: document.querySelector('#agentesConteudo td[data-tag="tipo"]')?.textContent,
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
// Os scripts que a tela carrega saem do proprio index.html: uma lista digitada aqui
// deixaria de ver o arquivo novo (e deixou: o tarefas.js nasceu com 21 chaves "mortas").
const js = [...html.matchAll(/<script src="\.\/([^"]+)"/g)]
  .map(m => readFileSync(join(UI, m[1]), 'utf8')).join('\n');
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
console.log(`textos cravados (fora da fabrica): ${lista.length}  teto ${TETO_SPLASH_POR_ETAPA}`);
console.log(`isentos vistos: ${isentosVistos.size}  isentos sem uso: ${ISENTOS.size - isentosVistos.size}`);
console.log(`fabrica: ${existentes.size} chaves • pedidas pela tela ${pedidas.size} • ${porIdioma}`);
console.log(`laco: faltando ${faltando.length}${faltando.length ? ` (${faltando.join(', ')})` : ''} • mortas ${mortas.length}${mortas.length ? ` (${mortas.join(', ')})` : ''} • pt vazio ${ptVazio.length}`);
console.log(`troca de idioma (en -> botao -> pt): ${trocaOk ? 'ok' : `FALHA ${trocaDetalhe}`}`);
const ok = trocaOk && lista.length <= TETO_SPLASH_POR_ETAPA && !faltando.length && !mortas.length && !ptVazio.length;
process.exit(ok ? 0 : 1);
