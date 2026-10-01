// Prova da navegacao e das telas do Command Center, EXERCITANDO no Chromium (a lei da casa:
// interface so se prova exercitando). Clica cada botao do menu, confere que a tela certa e a
// unica visivel, confere os numeros contra os JSON gerados, e captura cada tela.
//
// A UI e servida do disco por page.route (sem servidor), e o Tauri e um stub: o invoke
// registra o que a tela pediu, e a grade do terminal reproduzida e uma que SAIU do motor
// real (tests/desktop/dados/grade_*.json, gravada pelo exemplo `retrato` do
// phxclaw-terminal). O que este roteiro NAO prova: o PTY atras da ponte -- isso e o
// desktop_e2e.py, com o binario Tauri de verdade.
//
// Uso: PLAYWRIGHT_BROWSERS_PATH=/opt/pw-browsers node tests/desktop/ui_navegacao.mjs [DIR_DA_UI]
// Sai 0 so se todas as checagens passarem; capturas em tests/desktop/out/ui_*.png.
import { createRequire } from 'node:module';
import { readFileSync, existsSync, mkdirSync } from 'node:fs';
import { dirname, join, extname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const require = createRequire(import.meta.url);
const { chromium } = require('/opt/node22/lib/node_modules/playwright');
const AQUI = dirname(fileURLToPath(import.meta.url));
const RAIZ = resolve(AQUI, '../..');
const UI = resolve(process.argv[2] || join(RAIZ, 'apps/phxclaw-ui'));
const OUT = join(AQUI, 'out');
mkdirSync(OUT, { recursive: true });
const ORIGEM = 'http://phxclaw.local';
const TIPOS = { '.html': 'text/html', '.js': 'text/javascript', '.css': 'text/css', '.svg': 'image/svg+xml', '.json': 'application/json', '.woff2': 'font/woff2', '.png': 'image/png' };

const lerAsset = nome => JSON.parse(readFileSync(join(RAIZ, 'apps/phxclaw-ui/assets', nome), 'utf8'));
const grade = JSON.parse(readFileSync(join(AQUI, 'dados/grade_bash.json'), 'utf8'));

const checagens = [];
function check(nome, ok, detalhe = '') {
  checagens.push(!!ok);
  console.log(`${ok ? 'ok   ' : 'FALHA '}${nome}${detalhe ? ` :: ${detalhe}` : ''}`);
}

// Stub do window.__TAURI__: o mesmo formato que o withGlobalTauri expoe.
function stubTauri(grade) {
  const ouvintes = {};
  window.__chamadas = [];
  const emitir = (ev, payload) => (ouvintes[ev] || []).forEach(f => f({ payload }));
  window.__emitir = emitir;
  window.__TAURI__ = {
    event: { listen: async (ev, f) => { (ouvintes[ev] ||= []).push(f); return () => {}; } },
    core: {
      invoke: async (cmd, args) => {
        window.__chamadas.push({ cmd, args });
        switch (cmd) {
          case 'host_status':
            return { version: '0.0.0-stub', session_uuid: '01900000-0000-7000-8000-000000000000', policy: {}, live_bus: { receiver_count: 1 }, api: null };
          case 'verify_evidence': return { valid: true, records: 1 };
          case 'events_snapshot': return [];
          case 'terminal_abrir': {
            const id = `t${window.__chamadas.length}`;
            // A grade chega ANTES de o invoke voltar, como pode acontecer no host real.
            emitir('terminal_grade', { id, ...grade });
            return { id, pid: 4242, programa: args.programa, cwd: '/projeto' };
          }
          case 'terminal_escrever': return true;
          case 'terminal_redimensionar': return null;
          case 'terminal_rolar': return null;
          case 'terminal_fechar': return null;
          default: throw new Error(`comando sem stub: ${cmd}`);
        }
      },
    },
  };
}

async function abrirPagina(browser, { semEquipe = false } = {}) {
  const page = await browser.newPage({ viewport: { width: 1560, height: 960 } });
  const erros = [];
  page.on('pageerror', e => erros.push(String(e)));
  page.on('console', m => { if (m.type() === 'error') erros.push(m.text()); });
  await page.route(`${ORIGEM}/**`, route => {
    const caminho = decodeURIComponent(new URL(route.request().url()).pathname);
    const arq = join(UI, caminho === '/' ? 'index.html' : caminho);
    if ((semEquipe && caminho.endsWith('/equipe.json')) || !arq.startsWith(UI) || !existsSync(arq)) {
      return route.fulfill({ status: 404, body: 'nao existe' });
    }
    return route.fulfill({ status: 200, body: readFileSync(arq), contentType: TIPOS[extname(arq)] || 'application/octet-stream' });
  });
  await page.addInitScript(stubTauri, grade);
  await page.goto(`${ORIGEM}/index.html?screen=dashboard`);
  return { page, erros };
}

const visiveis = page => page.$$eval('section.tela', ts => ts.filter(t => !t.hidden && getComputedStyle(t).display !== 'none').map(t => t.dataset.tela));

const browser = await chromium.launch();
try {
  const { page, erros } = await abrirPagina(browser);
  const botoes = await page.$$eval('.nav[data-tela]', bs => bs.map(b => b.dataset.tela));
  check('menu tem as cinco telas', JSON.stringify(botoes) === JSON.stringify(['geral', 'agentes', 'ide', 'ferramentas', 'absorcao']), botoes.join(','));

  for (const tela of botoes) {
    await page.click(`.nav[data-tela="${tela}"]`);
    await page.waitForTimeout(400);
    const vis = await visiveis(page);
    const atual = await page.$eval('.nav[aria-current="page"]', b => b.dataset.tela).catch(() => null);
    check(`clicar "${tela}" deixa so a tela ${tela} visivel`, vis.length === 1 && vis[0] === tela && atual === tela, `visiveis=${vis} atual=${atual}`);
    if (tela !== 'ide') await page.screenshot({ path: join(OUT, `ui_${tela}.png`) });
  }

  // Numeros: cada um tem de bater com o JSON gerado; nenhum selo digitado.
  const equipe = lerAsset('equipe.json');
  const ferramentas = lerAsset('ferramentas.json');
  const absorcao = lerAsset('absorcao.json');
  const selos = await page.$$eval('.nav b', bs => bs.map(b => [b.id, b.textContent.trim()]));
  const seloDe = id => (selos.find(s => s[0] === id) || [])[1];
  check('selo Agentes = total do equipe.json', seloDe('seloAgentes') === String(equipe.total), `${seloDe('seloAgentes')} / ${equipe.total}`);
  check('selo Ferramentas = total do ferramentas.json', seloDe('seloFerramentas') === String(ferramentas.total), `${seloDe('seloFerramentas')} / ${ferramentas.total}`);
  check('nenhum selo sem id (numero digitado)', selos.every(([id]) => id), JSON.stringify(selos));

  await page.click('.nav[data-tela="agentes"]');
  await page.waitForTimeout(200);
  const fichasAgentes = await page.$$eval('#agentesConteudo .ficha', f => f.length);
  const gruposAgentes = await page.$$eval('#agentesConteudo .grupo', g => g.length);
  check('Agentes: uma ficha por papel do equipe.json', fichasAgentes === equipe.total, `${fichasAgentes} / ${equipe.total}`);
  check('Agentes: um grupo por macroarea', gruposAgentes === equipe.macroareas.length, `${gruposAgentes} / ${equipe.macroareas.length}`);
  await page.fill('#agentesFiltro', 'humano');
  const filtradas = await page.$$eval('#agentesConteudo .ficha', f => f.length);
  check('Agentes: o filtro reduz as fichas', filtradas > 0 && filtradas < equipe.total, `${filtradas}`);
  await page.fill('#agentesFiltro', '');

  await page.click('.nav[data-tela="ferramentas"]');
  await page.waitForTimeout(200);
  const fichasF = await page.$$eval('#ferramentasConteudo .ficha', f => f.length);
  const gruposF = await page.$$eval('#ferramentasConteudo .grupo', g => g.length);
  const nGrupos = new Set(ferramentas.ferramentas.map(f => f.grupo)).size;
  check('Ferramentas: uma ficha por ferramenta montada', fichasF === ferramentas.total, `${fichasF} / ${ferramentas.total}`);
  check('Ferramentas: agrupadas por capacidade', gruposF === nGrupos, `${gruposF} / ${nGrupos}`);

  await page.click('.nav[data-tela="absorcao"]');
  await page.waitForTimeout(200);
  const produtos = await page.$$eval('#absorcaoConteudo .produto', p => p.length);
  check('Absorcao: um cartao por produto do absorcao.json', produtos === Object.keys(absorcao).length, `${produtos}`);

  // IDE: abrir bash, ver a grade desenhada, digitar e conferir o que foi pedido ao host.
  await page.click('.nav[data-tela="ide"]');
  await page.waitForTimeout(200);
  await page.click('#ideAbrirBash');
  await page.waitForTimeout(400);
  const abriu = await page.evaluate(() => window.__chamadas.find(c => c.cmd === 'terminal_abrir'));
  check('IDE: + TERMINAL BASH pede terminal_abrir bash com o tamanho que cabe', abriu && abriu.args.programa === 'bash' && abriu.args.colunas > 20 && abriu.args.linhas > 5, JSON.stringify(abriu?.args));
  const pintados = await page.evaluate(() => {
    const c = document.getElementById('termCanvas');
    const d = c.getContext('2d').getImageData(0, 0, c.width, c.height).data;
    let n = 0;
    for (let i = 0; i < d.length; i += 4) if (d[i] > 150 || d[i + 1] > 150) n++;
    return n;
  });
  check('IDE: a grade do motor foi desenhada no canvas (pixels de texto)', pintados > 2000, `${pintados} px claros`);
  const status = await page.textContent('#ideStatus');
  check('IDE: status mostra pid e tamanho da grade', status.includes('pid 4242') && status.includes(`${grade.colunas}×${grade.linhas}`), status);
  await page.click('#termCanvas');
  await page.keyboard.type('ls');
  await page.keyboard.press('Enter');
  await page.keyboard.press('Control+c');
  await page.keyboard.press('ArrowUp');
  await page.waitForTimeout(200);
  const teclas = await page.evaluate(() => window.__chamadas.filter(c => c.cmd === 'terminal_escrever').map(c => c.args.tecla));
  const resumo = teclas.map(t => `${t.ctrl ? 'C-' : ''}${t.tecla}`).join(' ');
  check('IDE: teclas vao ao host como {tecla, ctrl...} (traducao no motor)', resumo === 'l s Enter C-c ArrowUp', resumo);
  const sinteticas = await page.evaluate(() => {
    const antes = window.__chamadas.length;
    document.getElementById('termCanvas').dispatchEvent(new KeyboardEvent('keydown', { key: 'x', bubbles: true }));
    return window.__chamadas.length - antes;
  });
  check('IDE: tecla sintetica (nao isTrusted) nao chega ao shell', sinteticas === 0, `${sinteticas} chamadas`);
  await page.screenshot({ path: join(OUT, 'ui_ide.png') });
  await page.click('#ideFechar');
  await page.waitForTimeout(200);
  const fechou = await page.evaluate(() => window.__chamadas.some(c => c.cmd === 'terminal_fechar'));
  check('IDE: FECHAR TERMINAL pede terminal_fechar', fechou);
  check('sem erro de JavaScript na pagina', erros.length === 0, erros.join(' | ').slice(0, 300));
  await page.close();

  // Arquivo ausente: a tela diz que nao existe, e nao mostra numero.
  const { page: p2 } = await abrirPagina(browser, { semEquipe: true });
  await p2.click('.nav[data-tela="agentes"]');
  await p2.waitForTimeout(400);
  const aviso = await p2.textContent('#agentesResumo');
  const fichas2 = await p2.$$eval('#agentesConteudo .ficha', f => f.length);
  const selo2 = await p2.textContent('#seloAgentes');
  check('sem equipe.json: aviso, zero fichas e selo vazio', aviso.includes('não existe') && fichas2 === 0 && selo2 === '', `${aviso.slice(0, 60)} / ${fichas2} / "${selo2}"`);
  await p2.screenshot({ path: join(OUT, 'ui_agentes_sem_equipe.png') });
} catch (e) {
  check('roteiro', false, String(e).slice(0, 300));
} finally {
  await browser.close();
}
console.log(`placar: ${checagens.filter(Boolean).length}/${checagens.length}`);
process.exit(checagens.length && checagens.every(Boolean) ? 0 : 1);
