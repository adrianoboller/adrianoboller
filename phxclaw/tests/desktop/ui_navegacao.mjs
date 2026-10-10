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
import { cabecalhosDaUi, avisoDoCoopSemTls, ROTA_POLITICA, POLITICA_PADRAO } from './seguranca.mjs';

const require = createRequire(import.meta.url);
const { chromium } = require('/opt/node22/lib/node_modules/playwright');
const AQUI = dirname(fileURLToPath(import.meta.url));
const RAIZ = resolve(AQUI, '../..');
const UI = resolve(process.argv[2] || join(RAIZ, 'apps/phxclaw-ui'));
const OUT = join(AQUI, 'out');
mkdirSync(OUT, { recursive: true });
const ORIGEM = 'http://phxclaw.local';
const TIPOS = { '.html': 'text/html', '.js': 'text/javascript', '.css': 'text/css', '.svg': 'image/svg+xml', '.json': 'application/json', '.woff2': 'font/woff2', '.png': 'image/png' };

// Le do MESMO diretorio que o navegador carrega: prova numa copia compara a copia com ela mesma
// (antes lia de RAIZ e a copia era medida contra os JSON da arvore real).
const lerAsset = nome => JSON.parse(readFileSync(join(UI, 'assets', nome), 'utf8'));
const grade = JSON.parse(readFileSync(join(AQUI, 'dados/grade_bash.json'), 'utf8'));

// A API de tarefas falsa: um estado de cada, com a data FORA de ordem, para a grade provar
// que ordena pela data (mais nova primeiro) e nao pela ordem em que a API entregou. A hora e
// (i*3) % n: 3 primo com n da hora distinta a cada tarefa (com % 7 e 8 estados, t0 e t7 empatavam).
const ESTADOS_T = ['pending', 'awaiting_approval', 'awaiting_input', 'running', 'completed', 'failed', 'cancelled', 'budget_exceeded'];
const TAREFAS = ESTADOS_T.map((status, i) => ({
  id: `t${i}`, objective: `objetivo da tarefa ${i}`, status, model: 'falso', plan: [], steps: [], artifacts: [],
  created_at: `2026-10-01T0${(i * 3) % ESTADOS_T.length}:00:00Z`, updated_at: '2026-10-01T09:00:00Z',
}));

// A tela Configuracao le a vista que saiu do motor real (a prova dela e o ui_config.mjs).
const VISTA_CONFIG = JSON.parse(readFileSync(join(AQUI, 'dados/config_vista.json'), 'utf8'));

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

// A CSP do aplicativo de mesa sai do proprio tauri.conf.json: uma copia aqui envelheceria.
// Os cabecalhos de seguranca do agente (pwa.rs), em TODA resposta: a tela se prova sob a CSP
// que o servidor manda de verdade, e cada violacao vira erro de console (defeito).
const CABECALHOS = cabecalhosDaUi();
const violacoesCsp = [];
const CSP_TAURI = JSON.parse(readFileSync(join(RAIZ, 'apps/phxclaw-desktop/src-tauri/tauri.conf.json'), 'utf8')).app.security.csp;

async function abrirPagina(browser, { semEquipe = false, sem = [], csp = null } = {}) {
  const page = await browser.newPage({ viewport: { width: 1560, height: 960 } });
  const erros = [];
  page.on('pageerror', e => erros.push(String(e)));
  page.on('console', m => { if (m.type() === 'error' && !avisoDoCoopSemTls(m.text())) erros.push(`${m.text()} @ ${m.location()?.url ?? ''}`); });
  page.on('console', m => { if (/Content Security Policy|Refused to/.test(m.text())) violacoesCsp.push(`${csp ? 'tauri' : 'servidor'}: ${m.text().slice(0, 200)}`); });
  await page.route(`${ORIGEM}/**`, route => {
    if (new URL(route.request().url()).pathname === ROTA_POLITICA) return route.fulfill({ status: 200, contentType: 'application/json', headers: CABECALHOS, body: POLITICA_PADRAO });
    const caminho = decodeURIComponent(new URL(route.request().url()).pathname);
    if (caminho.startsWith('/v1/')) {
      const m = caminho.match(/^\/v1\/tasks\/(t\d)$/);
      // A tela Fluxos pede a lista ao abrir: pasta vazia (o editor e provado no ui_fluxos.mjs).
      // A tela Insights pede /v1/insights ao abrir: retrato vazio valido (provado no ui_insights.mjs).
      const corpo = m ? TAREFAS.find(t => t.id === m[1]) : caminho === '/v1/tasks' ? TAREFAS : caminho === '/v1/config' ? VISTA_CONFIG
        : caminho === '/v1/fluxos' ? { fluxos: [], invalidos: [] }
        : caminho === '/v1/insights' ? { total: 0, terminadas: 0, taxa_falha: 0, falhas: 0, duracao: { p50_ms: null, p95_ms: null, amostras: 0 }, custo: { total: null, nao_medidas: 0 }, por_estado: {}, falhas_comuns: [], dias: [], fluxos: [], desde: null, ate: '2026-10-10T00:00:00Z' } : null;
      return route.fulfill({ status: corpo ? 200 : 404, contentType: 'application/json', headers: CABECALHOS, body: JSON.stringify(corpo ?? { error: 'nao existe' }) });
    }
    const arq = join(UI, caminho === '/' ? 'index.html' : caminho);
    if ((semEquipe && caminho.endsWith('/equipe.json')) || sem.some(n => caminho.endsWith(`/${n}`)) || !arq.startsWith(UI) || !existsSync(arq)) {
      return route.fulfill({ status: 404, headers: CABECALHOS, body: 'nao existe' });
    }
    return route.fulfill({ status: 200, body: readFileSync(arq), contentType: TIPOS[extname(arq)] || 'application/octet-stream',
      headers: csp && extname(arq) === '.html' ? { ...CABECALHOS, 'content-security-policy': csp } : CABECALHOS });
  });
  await page.addInitScript(stubTauri, grade);
  await page.addInitScript(() => { try { localStorage.setItem('phxclaw.token', 'token-de-teste'); } catch { /* sem armazenamento */ } });
  await page.goto(`${ORIGEM}/index.html?screen=dashboard`);
  return { page, erros };
}

const visiveis = page => page.$$eval('section.tela', ts => ts.filter(t => !t.hidden && getComputedStyle(t).display !== 'none').map(t => t.dataset.tela));

const browser = await chromium.launch();
try {
  const { page, erros } = await abrirPagina(browser);
  const botoes = await page.$$eval('.nav[data-tela]', bs => bs.map(b => b.dataset.tela));
  // Ordem das 4 areas da casca (SP000036 L1): Painel; Trabalho; Capacidades; Sistema. A ordem
  // do DOM e a visual, para o foco seguir o olho.
  // Dez desde a tela Conversa (projeto -> pedido -> cartao), primeira do grupo TRABALHO, antes
  // de Tarefas; Fluxos e Insights seguem no mesmo grupo.
  check('menu tem as dez telas', JSON.stringify(botoes) === JSON.stringify(['geral', 'conversa', 'tarefas', 'fluxos', 'insights', 'ide', 'agentes', 'ferramentas', 'absorcao', 'config']), botoes.join(','));

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

  // Casca (SP000036 L1): o topo e o rodape tambem sao tela. Todo texto com digito FORA das
  // <section class="tela"> tem de morar num [data-fonte] -- o digito solto no rodape e o selo
  // digitado de novo com outro nome, e as varreduras por tela nao o alcancam.
  // Prova real (02/10/2026): <b>451</b> no footer passava 58/58 aqui e 0 cravados no
  // textos_fora_da_fabrica (so letras contam la); com esta checagem, FALHA :: 451.
  const cascaSolta = await page.evaluate(() => {
    const fora = [];
    const w = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT);
    for (let n = w.nextNode(); n; n = w.nextNode()) {
      const t = n.textContent.trim();
      if (!t || !/\d/.test(t)) continue;
      const el = n.parentElement;
      // Os selos do menu (.nav b[id]) ja sao medidos contra o JSON pelas checagens de selo
      // acima -- selo sem id e o que reprova la; aqui so o que ninguem mede.
      if (!el || el.closest('section.tela, script, style, #splash, .nav b[id]')) continue;
      if (!el.closest('[data-fonte]')) fora.push(t);
    }
    return fora;
  });
  check('casca: nenhum numero fora de [data-fonte] no topo e no rodape', cascaSolta.length === 0, cascaSolta.join(' | ').slice(0, 200));

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
  // SP000036 L2: os seis cartoes de acao sao familias do ferramentas.json, com a MESMA conta
  // concedidas/total da malha; fundo neutro (nunca a cor da borda) e a cor de acao so nos tres
  // que tem acao. As execucoes recentes sao as 5 mais novas do stub de /v1/tasks (que chega
  // fora de ordem de proposito), com o rotulo de estado da fabrica, nunca o codigo.
  const fabL2 = lerAsset('textos.json').textos;
  const cartoes = await page.$$eval('#geralAcoes .acao-cartao', cs => cs.map(c => {
    const cs_ = getComputedStyle(c);
    return { acao: c.dataset.acao, grupo: c.dataset.grupo, conta: c.querySelector('em').textContent, nome: c.querySelector('b').textContent,
      borda: cs_.borderTopColor, fundo: cs_.backgroundColor, classe: [...c.classList].filter(k => k !== 'acao-cartao').join(' ') };
  }));
  const contaFamilia = g => { const fs = ferramentas.ferramentas.filter(f => f.grupo === g); return `${fs.filter(f => f.concedida).length}/${fs.length}`; };
  const contasOk = cartoes.length === 6 && cartoes.every(c => c.conta === contaFamilia(c.grupo)) && new Set(cartoes.map(c => c.grupo)).size === 6;
  check('Visao geral: 6 cartoes de acao = 6 familias do ferramentas.json, concedidas/total iguais ao JSON', contasOk, cartoes.map(c => `${c.acao}:${c.grupo}=${c.conta}`).join(' '));
  const nomesCartao = cartoes.map(c => c.nome);
  check('Visao geral: nome dos cartoes sai da fabrica (6 chaves painel.acao.*)', ['pesquisar', 'analisar', 'construir', 'testar', 'implantar', 'entregar'].every((k, i) => nomesCartao[i] === fabL2[`painel.acao.${k}`].pt), nomesCartao.join(','));
  const fundoCheio = cartoes.filter(c => c.fundo === c.borda || (c.fundo !== 'rgba(0, 0, 0, 0)' && c.fundo !== 'transparent'));
  const comCor = cartoes.filter(c => c.classe).map(c => `${c.acao}=${c.classe}`);
  check('Visao geral: cartoes so contorno (fundo neutro) e cor de acao em exatamente 3 (construir/testar/implantar)', fundoCheio.length === 0 && comCor.join(' ') === 'construir=inclui testar=consulta implantar=altera', `${comCor.join(' ')} cheios=${fundoCheio.length}`);
  const execs = await page.$$eval('#geralExecucoes .execucao', es => es.map(e => ({ id: e.dataset.id, estado: e.querySelector('.tarefa-estado').textContent, objetivo: e.querySelector('.objetivo').textContent })));
  const maisNovas = TAREFAS.slice().sort((a, b) => (a.created_at < b.created_at ? 1 : -1)).slice(0, 5);
  check('Visao geral: execucoes recentes = as 5 mais novas do stub de /v1/tasks, na ordem, estado pelo rotulo da fabrica',
    execs.length === 5 && execs.every((e, i) => e.id === maisNovas[i].id && e.objetivo === maisNovas[i].objective && e.estado === fabL2[`tarefas.estado.${maisNovas[i].status}`].pt),
    execs.map(e => `${e.id}:${e.estado}`).join(' | '));
  await page.click('#geralExecucoes .execucao');
  await page.waitForTimeout(600);
  const abriuExec = await page.evaluate(() => ({ tela: document.body.dataset.tela, titulo: document.querySelector('#tarefasDetalhe .tarefa-titulo')?.textContent, marcada: document.querySelector('#tarefasLista tbody tr.selecionada')?.dataset.id }));
  check('Visao geral: clicar uma execucao abre o MESMO detalhe da tela Tarefas', abriuExec.tela === 'tarefas' && abriuExec.titulo === maisNovas[0].objective && abriuExec.marcada === maisNovas[0].id, JSON.stringify(abriuExec));
  await page.click('.nav[data-tela="geral"]');
  await page.waitForTimeout(300);
  await page.click('#geralAcoes .acao-cartao[data-acao="testar"]');
  await page.waitForTimeout(300);
  const prefill = await page.evaluate(() => ({ tela: document.body.dataset.tela, valor: document.getElementById('tarefasObjetivo').value, foco: document.activeElement?.id }));
  check('Visao geral: o cartao leva a Tarefas com o objetivo-modelo da fabrica preenchido e o foco no campo (o envio e o formulario de la)',
    prefill.tela === 'tarefas' && prefill.valor === fabL2['painel.acao.testar_objetivo'].pt && prefill.foco === 'tarefasObjetivo', JSON.stringify(prefill).slice(0, 160));
  await page.$eval('#tarefasObjetivo', c => { c.value = ''; });
  await page.click('.nav[data-tela="geral"]');
  await page.waitForTimeout(300);
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

  await page.click('.nav[data-tela="geral"]');
  await page.waitForTimeout(300);
  // Barra de tres estados (agora so na Visao geral, que e o resumo; a Absorcao virou grade): a soma dos segmentos e o total do JSON, e cada estado tem FORMA
  // propria (cheio / hachurado / so contorno tracejado), conferida pelo estilo computado --
  // o que sobra em escala de cinza.
  const tri = await page.$$eval('#geralAbsorcaoLista .mini', ps => ps.map(p => ({
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
  check('Visao geral: segmentos agente/parcial/nao batem com o absorcao.json', tri.length && somaErrada.length === 0, somaErrada.join(','));
  const todos = tri.flatMap(t => t.segs);
  const formaOk = todos.every(s => (s.e === 'agente' && s.cheio && !s.img && s.borda === 'solid')
    || (s.e === 'parcial' && s.img && s.borda === 'solid')
    || (s.e === 'nao' && !s.cheio && !s.img && s.borda === 'dashed'));
  check('Visao geral: os tres estados diferem por forma (cheio / hachurado / tracejado), nao so cor', formaOk && new Set(todos.map(s => s.e)).size === 3, JSON.stringify(todos.slice(0, 3)));
  const proporcional = tri.every(t => {
    const tot = t.segs.reduce((a, s) => a + s.n, 0), largura = t.segs.reduce((a, s) => a + s.w, 0);
    return t.segs.every(s => Math.abs(s.w / largura - s.n / tot) < 0.06);
  });
  check('Visao geral: largura de cada segmento proporcional a contagem', proporcional);
  const legenda = await page.$$eval('#tela-absorcao .tela-head .tri-legenda .amostra', a => a.map(x => x.className));
  check('Absorcao: legenda unica com as tres amostras de forma', legenda.length === 3, legenda.join(','));
  // ===== GRADES (phx-grid): as listagens viraram table view; cada checagem exercita a grade
  // de verdade (agrupa, filtra, ordena, troca o idioma, exporta) e confere contra o JSON.
  const gradeInfo = sel => page.$eval(sel, raiz => {
    const trs = [...raiz.querySelectorAll('.phx-tabela > tbody > tr')];
    const num = t => Number(String(t).replace(/\D/g, '')) || 0;
    const grupos = trs.filter(tr => tr.classList.contains('phx-grupo')).map(tr => ({
      rotulo: tr.querySelector('.phx-grupo-rotulo')?.textContent ?? '',
      valor: tr.querySelector('.grade-valor-grupo')?.textContent ?? null,
      n: num(tr.querySelector('.phx-grupo-conta')?.textContent),
    }));
    const dados = trs.filter(tr => !/(^|\s)phx-(grupo|grupo-rodape|detalhe|tr-vazia|preview)(\s|$)/.test(tr.className));
    const cel = (tr, tag) => tr.querySelector(`td[data-tag="${tag}"]`)?.textContent ?? null;
    // As celulas se acham pela tag da coluna (data-tag, que o phx-grid poe no <th> e no <td>).
    const tags = [...raiz.querySelectorAll('.phx-tabela > thead th[data-tag]:not(.phx-frow-cel)')].map(th => th.dataset.tag);
    return {
      grupos, n: dados.length,
      linhas: dados.map(tr => Object.fromEntries(tags.map(t => [t, cel(tr, t)]))),
      rodapes: trs.filter(tr => tr.classList.contains('phx-grupo-rodape')).map(tr => [...tr.querySelectorAll('td.phx-grupo-rod-cel')].map(td => td.textContent.trim())),
      cabecalhos: [...raiz.querySelectorAll('.phx-tabela > thead th[data-campo]:not(.phx-frow-cel) .phx-th-titulo')].map(e => e.firstChild?.textContent ?? ''),
      canto: raiz.querySelector('.phx-tfoot-canto')?.textContent ?? '',
    };
  });
  const fabTextos = lerAsset('textos.json').textos;
  // Os nomes de produto saem da propria tela (NOMES_PRODUTO do app.js): uma copia aqui seria
  // mais um lugar para divergir.
  const NOMES = await page.evaluate(() => NOMES_PRODUTO);

  await page.click('.nav[data-tela="agentes"]');
  await page.waitForTimeout(400);
  let ga = await gradeInfo('#agentesConteudo');
  const porMacro = Object.fromEntries(equipe.macroareas.map(m => [m.nome, m.total]));
  const somaGrupos = ga.grupos.reduce((a, g) => a + g.n, 0);
  const gruposBatem = ga.grupos.every(g => Object.entries(porMacro).some(([nome, n]) => g.rotulo.endsWith(nome) && g.n === n));
  check('Agentes: agrupado por macroarea da 11 grupos e a soma e 110, cada grupo com o total do equipe.json',
    ga.grupos.length === equipe.macroareas.length && somaGrupos === equipe.total && ga.n === equipe.total && gruposBatem,
    `${ga.grupos.length} grupos, soma ${somaGrupos}, ${ga.n} linhas, batem=${gruposBatem}`);
  check('Agentes: caixa de agrupamento mostra o chip da macroarea (arraste uma coluna)',
    (await page.$$eval('#agentesConteudo .phx-groupbox .phx-gpill', ps => ps.map(p => p.dataset.campo))).join() === 'macroarea');
  // Ordenacao: clicar o titulo de «Papel» ordena; a ordem dentro do primeiro grupo muda e fica crescente.
  const nomesAntes = ga.linhas.map(l => l.nome);
  await page.click('#agentesConteudo th[data-campo="nome"] .phx-th-titulo');
  await page.waitForTimeout(300);
  ga = await gradeInfo('#agentesConteudo');
  const nomesDepois = ga.linhas.map(l => l.nome);
  const primeiro = ga.grupos[0]?.n || 0;
  const g1 = nomesDepois.slice(0, primeiro);
  const crescente = g1.every((v, i) => i === 0 || g1[i - 1].localeCompare(v, 'pt', { sensitivity: 'base' }) <= 0);
  check('Agentes: clicar o cabecalho ordena (a ordem muda e o grupo fica crescente)', JSON.stringify(nomesAntes) !== JSON.stringify(nomesDepois) && crescente && ga.n === equipe.total,
    `${nomesDepois.slice(0, 3).join(' | ')}`);
  // Filtro: a busca global e a linha de filtro (auto filter row) reduzem as linhas.
  await page.fill('#agentesFiltro', 'humano');
  await page.waitForTimeout(500);
  const nBusca = (await gradeInfo('#agentesConteudo')).n;
  await page.fill('#agentesFiltro', '');
  await page.waitForTimeout(500);
  await page.fill('#agentesConteudo th.phx-frow-cel[data-campo="nucleo"] input', 'UX');
  await page.waitForTimeout(600);
  const nLinhaFiltro = (await gradeInfo('#agentesConteudo')).n;
  check('Agentes: a busca e a linha de filtro reduzem as linhas', nBusca > 0 && nBusca < equipe.total && nLinhaFiltro > 0 && nLinhaFiltro < equipe.total,
    `busca=${nBusca} linha-de-filtro=${nLinhaFiltro} de ${equipe.total}`);
  await page.screenshot({ path: join(OUT, 'ui_agentes_filtro.png') });
  await page.fill('#agentesConteudo th.phx-frow-cel[data-campo="nucleo"] input', '');
  await page.waitForTimeout(600);
  // Colunas escolhiveis: o seletor do rodape esconde e mostra.
  const visAntes = (await gradeInfo('#agentesConteudo')).cabecalhos.length;
  // Clique de gente (o Playwright confere que o alvo esta visivel e nao coberto): menu
  // cortado pela borda da grade reprova aqui, e nao derruba o roteiro.
  const clicar = sel => page.click(sel, { timeout: 3000 }).then(() => true, () => false);
  await clicar('#agentesConteudo .phx-colsel-btn');
  const escondeu = await clicar('#agentesConteudo .phx-colsel input[data-campo="missao"]');
  await page.waitForTimeout(300);
  const visDepois = (await gradeInfo('#agentesConteudo')).cabecalhos.length;
  const mostrou = escondeu && await clicar('#agentesConteudo .phx-colsel input[data-campo="missao"]');
  if (!mostrou) await page.evaluate(() => gradesDaTela.agentes.g.mostrarColuna('missao', true));
  await clicar('#agentesConteudo .phx-colsel-btn');
  await page.waitForTimeout(300);
  check('Agentes: o seletor de colunas esconde e mostra uma coluna', escondeu && mostrou && visDepois === visAntes - 1 && (await gradeInfo('#agentesConteudo')).cabecalhos.length === visAntes, `${visAntes} -> ${visDepois}`);
  // Exportar: gera arquivo de verdade, com uma linha por papel.
  // Download que nao vem em 5 s e reprovacao desta checagem, nao queda do roteiro.
  const baixar = sel => Promise.all([page.waitForEvent('download', { timeout: 5000 }).catch(() => null), page.click(sel)]).then(([d]) => d);
  const csv = await baixar('[data-grade-exporta="agentes:csv"]');
  const csvLinhas = csv ? readFileSync(await csv.path(), 'utf8').replace(/^\uFEFF/, '').trim().split(/\r?\n/) : [];
  const xlsx = await baixar('[data-grade-exporta="agentes:xlsx"]');
  const xlsxBytes = xlsx ? readFileSync(await xlsx.path()) : Buffer.alloc(0);
  check('Agentes: EXPORTAR gera CSV (cabecalho + 110 linhas) e XLSX (zip PK)',
    csv?.suggestedFilename() === 'phxclaw-agentes.csv' && csvLinhas.length === equipe.total + 1 && xlsx?.suggestedFilename() === 'phxclaw-agentes.xlsx' && xlsxBytes[0] === 0x50 && xlsxBytes[1] === 0x4b,
    `${csv?.suggestedFilename()} ${csvLinhas.length} linhas; ${xlsx?.suggestedFilename()} ${xlsxBytes.length} bytes`);
  // Idioma: PT -> EN muda os cabecalhos (pela chave) e nao muda o dado; o agrupamento sobrevive.
  const ptA = await gradeInfo('#agentesConteudo');
  await page.click('#trocarIdioma');
  await page.waitForTimeout(500);
  const enA = await gradeInfo('#agentesConteudo');
  await page.screenshot({ path: join(OUT, 'ui_agentes_en.png') });
  await page.click('#trocarIdioma');
  await page.waitForTimeout(500);
  const pt2A = await gradeInfo('#agentesConteudo');
  const cabEn = ['agentes.col.nome', 'agentes.col.nucleo', 'agentes.col.tipo'].map(k => fabTextos[k].en);
  const tiposEn = new Set(enA.linhas.map(l => l.tipo));
  const dadoA = l => l.map(x => [x.nome, x.cap, x.missao].join('|')).join('\n');
  check('Agentes: trocar o idioma muda os cabecalhos e o rotulo HUMANO/AGENTE, e nao muda nenhum dado',
    cabEn.every(c => enA.cabecalhos.includes(c)) && JSON.stringify(ptA.cabecalhos) === JSON.stringify(pt2A.cabecalhos) && ptA.cabecalhos.join() !== enA.cabecalhos.join()
    && [...tiposEn].every(t => [fabTextos['agentes.humano'].en, fabTextos['agentes.agente'].en].includes(t))
    && dadoA(ptA.linhas) === dadoA(enA.linhas) && dadoA(enA.linhas) === dadoA(pt2A.linhas) && enA.grupos.length === equipe.macroareas.length,
    `${enA.cabecalhos.slice(0, 4).join(',')} / ${[...tiposEn].join(',')}`);
  await page.screenshot({ path: join(OUT, 'ui_agentes.png') });

  await page.click('.nav[data-tela="ferramentas"]');
  await page.waitForTimeout(400);
  const gf = await gradeInfo('#ferramentasConteudo');
  const nCaps = new Set(ferramentas.ferramentas.map(f => f.capacidade)).size;
  check('Ferramentas: uma linha por ferramenta, agrupadas por capability exata', gf.n === ferramentas.total && gf.grupos.length === nCaps && gf.grupos.reduce((a, g) => a + g.n, 0) === ferramentas.total,
    `${gf.n} linhas / ${gf.grupos.length} grupos de ${nCaps}`);
  // A coluna «exige» de cada linha e a do JSON e a do grupo onde a linha esta (nada de linha no
  // grupo errado por um find() que parou na primeira).
  const exigeErrado = await page.$eval('#ferramentasConteudo', (raiz, esperado) => {
    const erros = [];
    let grupo = null;
    for (const tr of raiz.querySelectorAll('.phx-tabela > tbody > tr')) {
      if (tr.classList.contains('phx-grupo')) { grupo = tr.querySelector('.phx-grupo-rotulo').textContent.split(': ').pop(); continue; }
      const nome = tr.querySelector('td[data-tag="nome"]')?.textContent;
      const exige = tr.querySelector('td[data-tag="exige"]')?.textContent;
      if (!nome) continue;
      if (exige !== esperado[nome] || exige !== grupo) erros.push(`${nome}:${exige}@${grupo}`);
    }
    return erros;
  }, Object.fromEntries(ferramentas.ferramentas.map(f => [f.nome, f.capacidade])));
  const temExige = await page.$$eval('#ferramentasConteudo th[data-campo="exige"]', t => t.length);
  check('Ferramentas: a coluna «exige» mostra a capability de cada ferramenta, igual a do JSON e a do grupo', exigeErrado.length === 0 && temExige === 1, exigeErrado.slice(0, 4).join(' '));

  await page.click('.nav[data-tela="absorcao"]');
  await page.waitForTimeout(400);
  let gb = await gradeInfo('#absorcaoConteudo');
  const totalCaps = Object.values(absorcao).reduce((a, p) => a + p.total, 0);
  const rodapesOk = gb.grupos.length === Object.keys(absorcao).length && gb.grupos.every((g, i) => {
    const k = Object.keys(NOMES).find(x => NOMES[x] === g.valor);
    const p = absorcao[k];
    return p && g.n === p.total && JSON.stringify((gb.rodapes[i] || []).slice(-3)) === JSON.stringify([String(p.no_agente), String(p.parcial), String(p.nao)]);
  });
  check('Absorcao: agrupada por produto, rodape de cada grupo soma no agente/pela metade/nao igual ao absorcao.json',
    gb.n === totalCaps && rodapesOk, `${gb.n} linhas, ${gb.grupos.length} grupos, rodapes ${JSON.stringify(gb.rodapes).slice(0, 120)}`);
  const estadoErrado = gb.linhas.filter(l => ![fabTextos['absorcao.no_agente'].pt, fabTextos['absorcao.parcial'].pt, fabTextos['absorcao.nao'].pt].includes(l.estado));
  check('Absorcao: o estado aparece pelo rotulo da fabrica (valor substituido), nunca o codigo', estadoErrado.length === 0, estadoErrado.slice(0, 3).map(l => l.estado).join(','));
  await page.screenshot({ path: join(OUT, 'ui_absorcao.png') });
  await page.click('#absorcaoPorEstado');
  await page.waitForTimeout(400);
  gb = await gradeInfo('#absorcaoConteudo');
  const porEstado = { agente: 0, parcial: 0, nao: 0 };
  for (const p of Object.values(absorcao)) { porEstado.agente += p.no_agente; porEstado.parcial += p.parcial; porEstado.nao += p.nao; }
  const rotEstado = { [fabTextos['absorcao.no_agente'].pt]: 'agente', [fabTextos['absorcao.parcial'].pt]: 'parcial', [fabTextos['absorcao.nao'].pt]: 'nao' };
  check('Absorcao: AGRUPAR POR ESTADO da 3 grupos com as contagens do JSON', gb.grupos.length === 3 && gb.grupos.every(g => porEstado[rotEstado[g.valor]] === g.n),
    gb.grupos.map(g => `${g.valor}=${g.n}`).join(' '));
  await page.screenshot({ path: join(OUT, 'ui_absorcao_por_estado.png') });
  await page.click('#absorcaoPorProduto');
  await page.click('#absorcaoVerCubo');
  await page.waitForTimeout(500);
  const cuboT = await page.$eval('#absorcaoCubo', raiz => ({
    visivel: !raiz.hidden,
    linhas: [...raiz.querySelectorAll('table tbody tr')].map(tr => [...tr.children].map(td => td.textContent.trim())),
    cab: [...raiz.querySelectorAll('table thead th')].map(th => th.textContent.trim()),
  }));
  const totaisCubo = Object.fromEntries(cuboT.linhas.filter(l => l.length > 1).map(l => [l[0], l[l.length - 1]]));
  const cuboOk = cuboT.visivel && Object.entries(absorcao).every(([k, p]) => totaisCubo[NOMES[k]] === String(p.total)) && Object.values(totaisCubo).includes(String(totalCaps));
  check('Absorcao: VISAO CUBO produto x estado, total de cada produto e o geral iguais ao JSON', cuboOk, JSON.stringify(totaisCubo).slice(0, 200));
  await page.screenshot({ path: join(OUT, 'ui_absorcao_cubo.png') });
  await page.click('#absorcaoTranspor');
  await page.waitForTimeout(300);
  const cabT = await page.$$eval('#absorcaoCubo table thead th', t => t.map(x => x.textContent.trim()));
  check('Absorcao: TRANSPOR troca os eixos do cubo (produtos viram colunas)', Object.values(NOMES).every(n => cabT.includes(n)), cabT.join(','));
  await page.click('#absorcaoVerCubo');
  await page.waitForTimeout(200);

  // Numero so de JSON gerado, tambem nas grades: todo texto com numero das tres telas de
  // listagem mora dentro de um [data-fonte] (a grade inteira e o resumo dizem de onde vieram).
  for (const tela of ['agentes', 'ferramentas', 'absorcao']) {
    await page.click(`.nav[data-tela="${tela}"]`);
    await page.waitForTimeout(300);
    const r = await page.evaluate(t => {
      const raiz = document.getElementById(`tela-${t}`);
      const num = /\d/;
      const fora = [];
      let dentro = 0;
      const w = document.createTreeWalker(raiz, NodeFilter.SHOW_TEXT);
      for (let n = w.nextNode(); n; n = w.nextNode()) {
        const tx = n.textContent.trim();
        if (!tx || !num.test(tx)) continue;
        if (n.parentElement.closest('[data-fonte]')) dentro++; else fora.push(tx);
      }
      return { fora, dentro };
    }, tela);
    check(`${tela}: nenhum numero fora de [data-fonte] (${r.dentro} dentro)`, r.fora.length === 0 && r.dentro > 0, r.fora.join(' | ').slice(0, 200));
  }

  // Tarefas: a lista e grade -- status pelo rotulo da fabrica (valores), mais nova primeiro
  // (ordem pela data, que a API entrega fora de ordem aqui de proposito) e o navegador de
  // registros abrindo o detalhe.
  await page.click('.nav[data-tela="tarefas"]');
  await page.waitForTimeout(800);
  const gt = await gradeInfo('#tarefasLista');
  const esperadoT = TAREFAS.slice().sort((a, b) => (a.created_at < b.created_at ? 1 : -1));
  const statusOk = gt.n === TAREFAS.length && gt.linhas.every((l, i) => l.status === fabTextos[`tarefas.estado.${esperadoT[i].status}`].pt);
  const ordemOk = JSON.stringify(gt.linhas.map(l => l.objetivo)) === JSON.stringify(esperadoT.map(t => t.objective));
  check('Tarefas: o status aparece pelo rotulo da fabrica (nunca o codigo) e a mais nova vem primeiro', statusOk && ordemOk,
    gt.linhas.slice(0, 3).map(l => `${l.status}/${l.objetivo}`).join(' | '));
  const formas = await page.$$eval('#tarefasLista td[data-tag="status"]', tds => tds.map(td => [...td.classList].find(c => c.startsWith('phx-est-st-')) || ''));
  check('Tarefas: cada status tem a forma da sua familia (ok, espera, erro, anda)', formas.length === TAREFAS.length && formas.every(Boolean), formas.join(','));
  const proximo = await page.$('#tarefasLista .phx-nav-b[data-nav="prox"]');
  if (proximo) {
    await proximo.click();
    await page.waitForTimeout(600);
    await page.click('#tarefasLista .phx-nav-b[data-nav="prox"]');
    await page.waitForTimeout(800);
  }
  const titulo = await page.textContent('#tarefasDetalhe .tarefa-titulo').catch(() => null);
  const marcada = await page.$$eval('#tarefasLista tbody tr.selecionada', trs => trs.map(t => t.dataset.id));
  check('Tarefas: o navegador de registros anda e abre o detalhe da tarefa do registro', titulo === esperadoT[1].objective && marcada.join() === esperadoT[1].id,
    `${titulo} / ${marcada}`);
  await page.screenshot({ path: join(OUT, 'ui_tarefas.png') });
  await page.click('#trocarIdioma');
  await page.waitForTimeout(800);
  const gtEn = await gradeInfo('#tarefasLista');
  await page.click('#trocarIdioma');
  await page.waitForTimeout(600);
  check('Tarefas: trocar o idioma troca o rotulo do status e o cabecalho, e nao o objetivo',
    gtEn.linhas.every((l, i) => l.status === fabTextos[`tarefas.estado.${esperadoT[i].status}`].en) && gtEn.cabecalhos[0] === fabTextos['tarefas.col.estado'].en
    && JSON.stringify(gtEn.linhas.map(l => l.objetivo)) === JSON.stringify(esperadoT.map(t => t.objective)), gtEn.linhas.slice(0, 2).map(l => l.status).join(','));
  // Troca de idioma com a tela aberta: o que o JS desenhou se redesenha pela CHAVE, e volta.
  await page.click('.nav[data-tela="absorcao"]');
  await page.waitForTimeout(300);
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
    geral: '#kernelVersion, #hostSession, #liveReceivers, #geralAgentes, #geralFerramentas, #geralAbsorcao, #geralMacro .macro > span, #geralMacro .macro b, #geralCapacidades code, #geralCapacidades em, #geralAbsorcaoLista .mini > span, #geralAbsorcaoLista .seg em, #geralAbsorcaoLista .mini b',
    agentes: '#agentesConteudo td[data-tag="nome"], #agentesConteudo td[data-tag="cap"], #agentesConteudo td[data-tag="missao"], #agentesConteudo .phx-grupo-conta',
    ferramentas: '#ferramentasConteudo td[data-tag="nome"], #ferramentasConteudo td[data-tag="exige"], #ferramentasConteudo td[data-tag="descricao"], #ferramentasConteudo .phx-grupo-conta',
    absorcao: '#absorcaoConteudo td[data-tag="capacidade"], #absorcaoConteudo .grade-valor-grupo, #absorcaoConteudo .phx-grupo-conta, #absorcaoConteudo td.phx-grupo-rod-cel',
  };
  const ROTULO_JS = {
    geral: '#geralAgentesNota, #geralFerramentasNota, #geralAbsorcaoNota, #geralAbsorcaoLista .tri-legenda, #apiEndpoint, #evidenceState, #hostPolicy',
    agentes: '#agentesResumo, #agentesConteudo .phx-th-titulo, #agentesConteudo td[data-tag="tipo"]',
    ferramentas: '#ferramentasResumo, #ferramentasConteudo .phx-th-titulo, #ferramentasConteudo td[data-tag="concessao"]',
    absorcao: '#absorcaoResumo, #absorcaoLegenda, #absorcaoConteudo .phx-th-titulo, #absorcaoConteudo td[data-tag="estado"]',
  };
  const foto = tela => page.evaluate(([tela, dado, rot]) => {
    const sec = document.getElementById(`tela-${tela}`);
    const textos = sel => [...sec.querySelectorAll(sel)].map(e => e.textContent);
    return { chaves: [...sec.querySelectorAll('[data-txt]')].map(e => [e.dataset.txt, e.textContent]), dado: textos(dado), rotulo: textos(rot) };
  }, [tela, DADO[tela], ROTULO_JS[tela]]);
  for (const tela of ['geral', 'agentes', 'ferramentas', 'absorcao']) {
    await page.click(`.nav[data-tela="${tela}"]`);
    await page.waitForTimeout(300);
    const pt1 = await foto(tela);
    await page.click('#trocarIdioma');
    await page.waitForTimeout(400);
    const en = await foto(tela);
    if (tela === 'ferramentas') await page.screenshot({ path: join(OUT, 'ui_ferramentas_en.png') });
    await page.click('#trocarIdioma');
    await page.waitForTimeout(400);
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
  // Quatro avisos: macroareas, malha, absorcao e (SP000036 L2) os cartoes de acao, que perdem a
  // fonte junto com a malha -- cartao sem ferramentas.json nao mostra 0/0 inventado.
  check('sem os tres JSON: Visao geral mostra travessao e quatro avisos, sem barra', vazio.nums.every(n => n === '—') && vazio.avisos === 4 && vazio.barra, JSON.stringify(vazio));
  await p3.screenshot({ path: join(OUT, 'ui_geral_sem_json.png') });

  // Sob a CSP do Tauri (a do servidor + o canal ipc:, teste Rust
  // a_csp_do_desktop_e_a_do_servidor_e_o_inspetor_fica_fora): a grade tem de funcionar
  // inteira e sem violacao nenhuma. Ate 09/10/2026 o desktop tinha style-src 'self' e o
  // phx-grid perdia calado os style="" que monta (recuo de grupo, linha do cubo); a CSP de
  // agora aceita estilo em linha (pwa.rs diz o preco) e continua sem script em linha.
  const { page: p4, erros: e4 } = await abrirPagina(browser, { csp: CSP_TAURI });
  await p4.evaluate(() => { window.__csp = []; document.addEventListener('securitypolicyviolation', e => window.__csp.push(e.effectiveDirective)); });
  await p4.click('.nav[data-tela="agentes"]');
  await p4.waitForTimeout(500);
  await p4.click('.nav[data-tela="absorcao"]');
  await p4.click('#absorcaoVerCubo');
  await p4.waitForTimeout(500);
  const sobCsp = await p4.evaluate(() => ({
    agentes: [...document.querySelectorAll('#agentesConteudo .phx-tabela > tbody > tr')].filter(tr => !tr.className).length,
    cubo: document.querySelectorAll('#absorcaoCubo table tbody tr').length,
    violacoes: window.__csp,
  }));
  const scripts = sobCsp.violacoes.filter(v => v.startsWith('script'));
  check('sob a CSP do Tauri: a grade de agentes e o cubo desenham, zero violacao',
    sobCsp.agentes === equipe.total && sobCsp.cubo > Object.keys(absorcao).length && scripts.length === 0 && sobCsp.violacoes.length === 0 && !e4.some(x => /Refused to/.test(x)),
    `${sobCsp.agentes} linhas, cubo ${sobCsp.cubo} linhas, violacoes: ${JSON.stringify(sobCsp.violacoes.reduce((a, v) => ({ ...a, [v]: (a[v] || 0) + 1 }), {}))}`);
  check('zero violacao de CSP em todas as paginas (servidor e Tauri)', violacoesCsp.length === 0, violacoesCsp.slice(0, 5).join(' | '));
} catch (e) {
  check('roteiro', false, String(e).slice(0, 300));
} finally {
  await browser.close();
}
console.log(`placar: ${checagens.filter(Boolean).length}/${checagens.length}`);
process.exit(checagens.length && checagens.every(Boolean) ? 0 : 1);
