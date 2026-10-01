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

async function abrirPagina(browser, { semEquipe = false, sem = [] } = {}) {
  const page = await browser.newPage({ viewport: { width: 1560, height: 960 } });
  const erros = [];
  page.on('pageerror', e => erros.push(String(e)));
  page.on('console', m => { if (m.type() === 'error') erros.push(`${m.text()} @ ${m.location()?.url ?? ''}`); });
  await page.route(`${ORIGEM}/**`, route => {
    const caminho = decodeURIComponent(new URL(route.request().url()).pathname);
    const arq = join(UI, caminho === '/' ? 'index.html' : caminho);
    if ((semEquipe && caminho.endsWith('/equipe.json')) || sem.some(n => caminho.endsWith(`/${n}`)) || !arq.startsWith(UI) || !existsSync(arq)) {
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

  // Visao geral: nenhum numero solto. Todo texto com numero isolado (8 formatos, 7 TRUSTED,
  // 3x, 78%) e toda largura em estilo tem de morar dentro de um [data-fonte]; e os numeros
  // que estao la batem com os JSON.
  await page.click('.nav[data-tela="geral"]');
  await page.waitForTimeout(400);
  const soltos = await page.evaluate(() => {
    const raiz = document.getElementById('tela-geral');
    const num = /(^|[\s(•/])\d+([.,]\d+)?\s*(%|×|x)?(?=$|[\s)•/–-])/;
    const fora = [];
    const w = document.createTreeWalker(raiz, NodeFilter.SHOW_TEXT);
    for (let n = w.nextNode(); n; n = w.nextNode()) {
      const t = n.textContent.trim();
      if (t && num.test(t) && !n.parentElement.closest('[data-fonte]')) fora.push(t);
    }
    for (const e of raiz.querySelectorAll('[style]')) if (!e.closest('[data-fonte]')) fora.push(`style:${e.getAttribute('style')}`);
    return fora;
  });
  check('Visao geral: nenhum numero ou largura fora de [data-fonte]', soltos.length === 0, soltos.join(' | ').slice(0, 200));
  const geral = await page.evaluate(() => ({
    agentes: document.getElementById('geralAgentes').textContent,
    ferramentas: document.getElementById('geralFerramentas').textContent,
    macros: [...document.querySelectorAll('#geralMacro .macro b')].map(b => Number(b.textContent)),
    familias: [...document.querySelectorAll('#geralCapacidades em')].map(e => e.textContent),
    barra: document.querySelector('#geralFerramentasBarra span').style.width,
    mini: document.querySelectorAll('#geralAbsorcaoLista .mini .tri').length,
    fixos: document.getElementById('tela-geral').textContent.match(/formatos|TRUSTED|Retries|Mission Pipeline|Task Graph/g),
  }));
  const concF = ferramentas.ferramentas.filter(f => f.concedida).length;
  const familiasEsperadas = new Set(ferramentas.ferramentas.map(f => f.grupo)).size;
  const somaFam = geral.familias.reduce((a, t) => a + Number(t.split('/')[1]), 0);
  check('Visao geral: cartoes de agentes e ferramentas = JSON', geral.agentes === String(equipe.total) && geral.ferramentas === String(ferramentas.total), `${geral.agentes} ${geral.ferramentas}`);
  check('Visao geral: macroareas e familias de capability saem dos JSON', JSON.stringify(geral.macros) === JSON.stringify(equipe.macroareas.map(m => m.total)) && geral.familias.length === familiasEsperadas && somaFam === ferramentas.total, `${geral.macros.length} macro, ${geral.familias.length} fam, soma ${somaFam}`);
  check('Visao geral: barra de concessao = concedidas/total', Math.abs(parseFloat(geral.barra) - (concF / ferramentas.total) * 100) < 0.01 && geral.mini === Object.keys(absorcao).length, `${geral.barra} mini=${geral.mini}`);
  check('Visao geral: sem os textos de enfeite (formatos, TRUSTED, Retries, pipeline e task graph estaticos)', !geral.fixos, String(geral.fixos));
  await page.screenshot({ path: join(OUT, 'ui_geral.png'), fullPage: false });
  await page.$eval('#tela-geral', t => { t.scrollTop = t.scrollHeight; });
  await page.screenshot({ path: join(OUT, 'ui_geral_rolada.png') });
  await page.$eval('#tela-geral', t => { t.scrollTop = 0; });
  await page.click('.hero-actions [data-ir="ferramentas"]');
  await page.waitForTimeout(200);
  check('Visao geral: VER FERRAMENTAS abre a tela Ferramentas', (await visiveis(page)).join() === 'ferramentas');
  const acoesCheias = await page.$$eval('.acao', bs => bs.filter(b => { const c = getComputedStyle(b).backgroundColor; return c !== 'rgba(0, 0, 0, 0)' && c !== 'transparent'; }).map(b => b.textContent));
  check('acoes so contorno (fundo transparente fora do hover)', acoesCheias.length === 0, acoesCheias.join(','));

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
  const nGrupos = new Set(ferramentas.ferramentas.map(f => f.capacidade)).size;
  check('Ferramentas: uma ficha por ferramenta montada', fichasF === ferramentas.total, `${fichasF} / ${ferramentas.total}`);
  check('Ferramentas: um grupo por capability exata', gruposF === nGrupos, `${gruposF} / ${nGrupos}`);
  // Cada ficha mostra a capability que exige, e ela e a do grupo onde esta (nada de ficha no
  // grupo errado por um find() que parou na primeira).
  const exigencias = await page.$$eval('#ferramentasConteudo .grupo', gs => gs.map(g => ({
    cap: g.dataset.capacidade,
    titulo: g.querySelector('header code.cap')?.textContent,
    fichas: [...g.querySelectorAll('.ficha')].map(f => [f.querySelector('b').textContent, f.querySelector('.exige code')?.textContent]),
  })));
  const esperado = new Map(ferramentas.ferramentas.map(f => [f.nome, f.capacidade]));
  const erradas = exigencias.flatMap(g => g.fichas.filter(([n, c]) => c !== g.cap || c !== esperado.get(n) || g.titulo !== g.cap).map(([n, c]) => `${n}:${c}@${g.cap}`));
  check('Ferramentas: cada ficha mostra a capability exigida, igual a do JSON e a do grupo', erradas.length === 0 && exigencias.length === nGrupos, erradas.join(' ').slice(0, 200));

  await page.click('.nav[data-tela="absorcao"]');
  await page.waitForTimeout(200);
  const produtos = await page.$$eval('#absorcaoConteudo .produto', p => p.length);
  check('Absorcao: um cartao por produto do absorcao.json', produtos === Object.keys(absorcao).length, `${produtos}`);
  // Barra de tres estados: a soma dos segmentos e o total do JSON, e cada estado tem FORMA
  // propria (cheio / hachurado / so contorno tracejado), conferida pelo estilo computado --
  // o que sobra em escala de cinza.
  const tri = await page.$$eval('#absorcaoConteudo .produto', ps => ps.map(p => ({
    k: p.dataset.produto,
    segs: [...p.querySelectorAll('.tri .seg')].map(s => {
      const cs = getComputedStyle(s);
      return { e: s.dataset.estado, n: Number(s.dataset.n), img: cs.backgroundImage !== 'none', borda: cs.borderTopStyle,
        cheio: cs.backgroundColor !== 'rgba(0, 0, 0, 0)' && cs.backgroundColor !== 'transparent', w: s.getBoundingClientRect().width };
    }),
  })));
  const somaErrada = tri.filter(t => {
    const p = absorcao[t.k];
    const de = e => t.segs.find(s => s.e === e)?.n || 0;
    return de('agente') !== p.no_agente || de('parcial') !== p.parcial || de('nao') !== p.nao;
  }).map(t => t.k);
  check('Absorcao: segmentos agente/parcial/nao batem com o absorcao.json', tri.length && somaErrada.length === 0, somaErrada.join(','));
  const todos = tri.flatMap(t => t.segs);
  const formaOk = todos.every(s => (s.e === 'agente' && s.cheio && !s.img && s.borda === 'solid')
    || (s.e === 'parcial' && s.img && s.borda === 'solid')
    || (s.e === 'nao' && !s.cheio && !s.img && s.borda === 'dashed'));
  check('Absorcao: os tres estados diferem por forma (cheio / hachurado / tracejado), nao so cor', formaOk && new Set(todos.map(s => s.e)).size === 3, JSON.stringify(todos.slice(0, 3)));
  const proporcional = tri.every(t => {
    const tot = t.segs.reduce((a, s) => a + s.n, 0), largura = t.segs.reduce((a, s) => a + s.w, 0);
    return t.segs.every(s => Math.abs(s.w / largura - s.n / tot) < 0.06);
  });
  check('Absorcao: largura de cada segmento proporcional a contagem', proporcional);
  const legenda = await page.$$eval('#tela-absorcao .tela-head .tri-legenda .amostra', a => a.map(x => x.className));
  check('Absorcao: legenda unica com as tres amostras de forma', legenda.length === 3, legenda.join(','));
  await page.screenshot({ path: join(OUT, 'ui_absorcao.png') });
  // Troca de idioma com a tela aberta: o que o JS desenhou se redesenha pela CHAVE, e volta.
  const textos = lerAsset('textos.json').textos;
  const lerRotulos = () => page.evaluate(() => ({
    legenda: [...document.querySelectorAll('#absorcaoLegenda .tri-legenda > span > span')].map(s => s.textContent),
    resumo: document.getElementById('absorcaoResumo').textContent,
  }));
  await page.click('#trocarIdioma');
  await page.waitForTimeout(200);
  const emEn = await lerRotulos();
  await page.click('#trocarIdioma');
  await page.waitForTimeout(200);
  const emPt = await lerRotulos();
  const esperaEn = ['absorcao.no_agente', 'absorcao.parcial', 'absorcao.nao'].map(k => textos[k].en);
  const esperaPt = ['absorcao.no_agente', 'absorcao.parcial', 'absorcao.nao'].map(k => textos[k].pt);
  check('Absorcao: trocar idioma redesenha a legenda pela chave, e volta', JSON.stringify(emEn.legenda) === JSON.stringify(esperaEn)
    && JSON.stringify(emPt.legenda) === JSON.stringify(esperaPt) && emEn.resumo !== emPt.resumo, `${emEn.legenda} / ${emPt.legenda}`);

  // Troca PT -> EN -> PT nas tres telas desenhadas pelo JS: rotulo muda pela chave, DADO nao
  // muda nunca (nome de papel, de ferramenta, capability, produto, numero).
  const fab = lerAsset('textos.json').textos;
  const DADO = {
    geral: '#geralAgentes, #geralFerramentas, #geralAbsorcao, #geralMacro .macro > span, #geralMacro .macro b, #geralCapacidades code, #geralCapacidades em, #geralAbsorcaoLista .mini > span, #geralAbsorcaoLista .seg em, #geralAbsorcaoLista .mini b',
    ferramentas: '#ferramentasConteudo header code.cap, #ferramentasConteudo .ficha b, #ferramentasConteudo .ficha p, #ferramentasConteudo .exige code',
    absorcao: '#absorcaoConteudo h2, #absorcaoConteudo .seg em, #absorcaoConteudo li',
  };
  const ROTULO_JS = {
    geral: '#geralAgentesNota, #geralFerramentasNota, #geralAbsorcaoNota, #geralAbsorcaoLista .tri-legenda',
    ferramentas: '#ferramentasResumo, #ferramentasConteudo .rotulo-cap, #ferramentasConteudo .exige > span, #ferramentasConteudo .marcas span',
    absorcao: '#absorcaoResumo, #absorcaoLegenda, #absorcaoConteudo .tri-legenda, #absorcaoConteudo small, #absorcaoConteudo summary',
  };
  const foto = tela => page.evaluate(([tela, dado, rot]) => {
    const sec = document.getElementById(`tela-${tela}`);
    const textos = sel => [...sec.querySelectorAll(sel)].map(e => e.textContent);
    return { chaves: [...sec.querySelectorAll('[data-txt]')].map(e => [e.dataset.txt, e.textContent]), dado: textos(dado), rotulo: textos(rot) };
  }, [tela, DADO[tela], ROTULO_JS[tela]]);
  for (const tela of ['geral', 'ferramentas', 'absorcao']) {
    await page.click(`.nav[data-tela="${tela}"]`);
    await page.waitForTimeout(250);
    const pt1 = await foto(tela);
    await page.click('#trocarIdioma');
    await page.waitForTimeout(250);
    const en = await foto(tela);
    if (tela === 'ferramentas') await page.screenshot({ path: join(OUT, 'ui_ferramentas_en.png') });
    await page.click('#trocarIdioma');
    await page.waitForTimeout(250);
    const pt2 = await foto(tela);
    const chavesOk = en.chaves.length > 0 && en.chaves.every(([k, t]) => t === fab[k].en) && pt2.chaves.every(([k, t]) => t === fab[k].pt);
    const dadoOk = pt1.dado.length > 0 && JSON.stringify(pt1.dado) === JSON.stringify(en.dado) && JSON.stringify(en.dado) === JSON.stringify(pt2.dado);
    const rotuloOk = pt1.rotulo.length > 0 && JSON.stringify(pt1.rotulo) === JSON.stringify(pt2.rotulo) && pt1.rotulo.every((t, i) => t !== en.rotulo[i]);
    const ruins = pt1.rotulo.filter((t, i) => t === en.rotulo[i]);
    check(`Idioma em ${tela}: PT->EN->PT muda ${en.chaves.length} rotulos do HTML e ${pt1.rotulo.length} do JS, e ${pt1.dado.length} dados ficam iguais`,
      chavesOk && dadoOk && rotuloOk, `chaves=${chavesOk} dado=${dadoOk} rotulo=${rotuloOk} ${ruins.slice(0, 3).join(' | ')}`);
  }

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

  const { page: p3 } = await abrirPagina(browser, { sem: ['equipe.json', 'ferramentas.json', 'absorcao.json'] });
  await p3.click('.nav[data-tela="geral"]');
  await p3.waitForTimeout(400);
  const vazio = await p3.evaluate(() => ({
    nums: ['geralAgentes', 'geralFerramentas', 'geralAbsorcao'].map(id => document.getElementById(id).textContent),
    avisos: document.querySelectorAll('#tela-geral .vazio.aviso').length,
    barra: document.getElementById('geralFerramentasBarra').hidden,
  }));
  check('sem os tres JSON: Visao geral mostra travessao e tres avisos, sem barra', vazio.nums.every(n => n === '—') && vazio.avisos === 3 && vazio.barra, JSON.stringify(vazio));
  await p3.screenshot({ path: join(OUT, 'ui_geral_sem_json.png') });
} catch (e) {
  check('roteiro', false, String(e).slice(0, 300));
} finally {
  await browser.close();
}
console.log(`placar: ${checagens.filter(Boolean).length}/${checagens.length}`);
process.exit(checagens.length && checagens.every(Boolean) ? 0 : 1);
