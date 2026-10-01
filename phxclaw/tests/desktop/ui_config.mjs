// Prova da tela Configuracao EXERCITANDO no Chromium, contra um stub com o MESMO contrato do
// `/v1/config` (crates/phxclaw-agent/src/config.rs): GET devolve a vista, PUT exige If-Match,
// responde 200 {revisao} / 409 {revisao_atual} / 422 {erros:[{chave,motivo}]}. A vista inicial
// SAIU do motor real (tests/desktop/dados/config_vista.json: `phxclaw config mostrar --json`
// com tres chaves na pasta, uma no ambiente e o token da API no ambiente).
//
// O que este roteiro NAO prova: a gravacao no disco -- isso e dos testes do agente
// (crates/phxclaw-agent/tests/config.rs), que chamam a mesma `definir`.
//
// Uso: PLAYWRIGHT_BROWSERS_PATH=/opt/pw-browsers node tests/desktop/ui_config.mjs [DIR_DA_UI]
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
const VISTA = JSON.parse(readFileSync(join(AQUI, 'dados/config_vista.json'), 'utf8'));
const fab = JSON.parse(readFileSync(join(UI, 'assets/textos.json'), 'utf8')).textos;
// O token da API e segredo no ambiente. O agente nao manda o valor; o stub MANDA (como um
// agente com defeito mandaria), para provar que a tela nunca o desenha mesmo assim.
const SEGREDO_NO_AMBIENTE = 'valor-do-segredo-que-nao-pode-vazar';

const checagens = [];
function check(nome, ok, detalhe = '') {
  checagens.push(!!ok);
  console.log(`${ok ? 'ok   ' : 'FALHA '}${nome}${detalhe ? ` :: ${detalhe}` : ''}`);
}

// O stub do agente, com estado: revisao, valores por escopo, e a validacao por tipo que o
// config-runtime faz (inteiro precisa ser inteiro, enum precisa estar nas opcoes).
function criarStub() {
  const st = { vista: JSON.parse(JSON.stringify(VISTA)), puts: [], forcarConflito: false };
  st.vista.chaves.find(c => c.chave === 'api.token').valor = SEGREDO_NO_AMBIENTE;
  const validar = (c, v) => {
    if (v === null) return null;
    if (c.tipo === 'booleano' && typeof v !== 'boolean') return 'esperado booleano';
    if (c.tipo === 'inteiro' && !Number.isInteger(v)) return `esperado inteiro, veio ${JSON.stringify(v)}`;
    if (c.tipo === 'real' && typeof v !== 'number') return 'esperado numero';
    if (c.tipo === 'enum' && !(c.opcoes || []).includes(v)) return `esperado um de ${(c.opcoes || []).join(', ')}`;
    if (c.tipo === 'lista' && !Array.isArray(v)) return 'esperada lista';
    return null;
  };
  st.tratar = (metodo, corpo, ifMatch) => {
    if (metodo === 'GET') return [200, st.vista];
    if (st.forcarConflito) { st.vista.revisao += 1; st.forcarConflito = false; }
    st.puts.push({ ifMatch, corpo });
    if (ifMatch === undefined) return [428, { error: 'falta If-Match' }];
    if (Number(ifMatch) !== st.vista.revisao) return [409, { revisao_atual: st.vista.revisao }];
    const porChave = new Map(st.vista.chaves.map(c => [c.chave, c]));
    const erros = [];
    for (const [k, v] of Object.entries(corpo.valores || {})) {
      const c = porChave.get(k);
      if (!c) { erros.push({ chave: k, motivo: 'chave desconhecida' }); continue; }
      if (!c.editavel) { erros.push({ chave: k, motivo: c.motivo_nao_editavel }); continue; }
      const m = validar(c, v);
      if (m) erros.push({ chave: k, motivo: m });
    }
    if (erros.length) return [422, { erros }];
    for (const [k, v] of Object.entries(corpo.valores)) {
      const c = porChave.get(k);
      c.valor = v === null ? c.padrao : v;
      c.origem = v === null ? 'padrao' : corpo.escopo;
    }
    st.vista.revisao += 1;
    return [200, { revisao: st.vista.revisao }];
  };
  return st;
}

async function abrir(browser, st, { token = 'token-de-teste', ponte = false } = {}) {
  const page = await browser.newPage({ viewport: { width: 1560, height: 960 } });
  const erros = [];
  page.on('pageerror', e => erros.push(String(e)));
  // 409 e 422 fazem parte do contrato: o navegador os registra como «Failed to load
  // resource», que nao e erro de JavaScript.
  page.on('console', m => { if (m.type() === 'error' && !/Failed to load resource: the server responded with a status of (403|409|422)/.test(m.text())) erros.push(m.text()); });
  await page.route(`${ORIGEM}/**`, route => {
    const req = route.request();
    const caminho = decodeURIComponent(new URL(req.url()).pathname);
    if (caminho === '/v1/config') {
      if (ponte) return route.fulfill({ status: 403, contentType: 'application/json', body: '{"error":"rota fora do controle remoto"}' });
      if (req.headers().authorization !== `Bearer ${token}`) return route.fulfill({ status: 401, contentType: 'application/json', body: '{"error":"token"}' });
      const [s, corpo] = st.tratar(req.method(), req.postData() ? JSON.parse(req.postData()) : null, req.headers()['if-match']);
      return route.fulfill({ status: s, contentType: 'application/json', body: JSON.stringify(corpo) });
    }
    if (caminho.startsWith('/v1/')) return route.fulfill({ status: 200, contentType: 'application/json', body: '[]' });
    const arq = join(UI, caminho === '/' ? 'index.html' : caminho);
    if (!arq.startsWith(UI) || !existsSync(arq)) return route.fulfill({ status: 404, body: 'nao existe' });
    return route.fulfill({ status: 200, body: readFileSync(arq), contentType: TIPOS[extname(arq)] || 'application/octet-stream' });
  });
  await page.addInitScript(t => { try { localStorage.setItem('phxclaw.token', t); } catch { /* */ } }, token);
  await page.goto(`${ORIGEM}/index.html?screen=dashboard#config`);
  await page.evaluate(() => idiomas.pronto);
  await page.waitForTimeout(700);
  return { page, erros };
}

const linha = (page, chave) => page.$(`#configConteudo tr:has(td[data-tag="chave"]:text-is("${chave}"))`);
const celula = async (page, chave, tag) => {
  const tr = await linha(page, chave);
  return tr ? tr.$(`td[data-tag="${tag}"]`) : null;
};
const textoCel = async (page, chave, tag) => (await (await celula(page, chave, tag))?.textContent())?.trim() ?? null;

const browser = await chromium.launch();
try {
  const st = criarStub();
  const { page, erros } = await abrir(browser, st);
  check('o menu tem a tela Configuracao e ela abre sozinha', await page.$eval('.nav[data-tela="config"]', b => b.getAttribute('aria-current') === 'page'));

  // Carregar: uma linha por chave, agrupada por secao, e o resumo com a revisao do agente.
  const info = await page.evaluate(() => ({
    linhas: document.querySelectorAll('#configConteudo .phx-tabela > tbody > tr:has(td[data-tag="chave"])').length,
    grupos: [...document.querySelectorAll('#configConteudo .phx-tabela > tbody > tr.phx-grupo')].map(tr => Number(tr.querySelector('.phx-grupo-conta').textContent.replace(/\D/g, ''))),
    status: document.getElementById('configStatus').textContent,
    cab: [...document.querySelectorAll('#configConteudo thead th[data-tag]:not(.phx-frow-cel) .phx-th-titulo')].map(e => e.firstChild.textContent),
  }));
  const secoes = new Set(VISTA.chaves.map(c => c.secao));
  check('carregar: uma linha por chave, agrupada por secao, soma = total de chaves', info.linhas === VISTA.chaves.length && info.grupos.length === secoes.size && info.grupos.reduce((a, b) => a + b, 0) === VISTA.chaves.length,
    `${info.linhas} linhas, ${info.grupos.length} secoes`);
  check('carregar: o resumo mostra a revisao e o arquivo que o agente deu', info.status.includes(String(VISTA.revisao)) && info.status.includes(VISTA.arquivos.pasta), info.status);
  check('colunas chave / valor / origem / padrao / descricao pelos rotulos da fabrica',
    ['config.col.chave', 'config.col.valor', 'config.col.origem', 'config.col.padrao', 'config.col.descricao'].every(k => info.cab.includes(fab[k].pt)), info.cab.join(','));
  await page.screenshot({ path: join(OUT, 'ui_config.png') });

  // Origem como badge por forma: ambiente (travado), pasta, padrao.
  const origens = await page.evaluate(() => Object.fromEntries(['agente.heartbeat_min', 'agente.lsp', 'agente.agentes_dir'].map(k => {
    const tr = [...document.querySelectorAll('#configConteudo tbody tr')].find(t => t.querySelector('td[data-tag="chave"]')?.textContent === k);
    const td = tr?.querySelector('td[data-tag="origem"]');
    return [k, td ? { texto: td.textContent, classe: [...td.classList].find(c => c.startsWith('phx-est-origem-')), contorno: getComputedStyle(td).outlineStyle } : null];
  })));
  check('origem: ambiente / pasta / padrao com o rotulo da fabrica e forma propria (cheio, cheio, tracejado)',
    origens['agente.heartbeat_min']?.texto === fab['config.origem.ambiente'].pt && origens['agente.heartbeat_min']?.classe === 'phx-est-origem-ambiente'
    && origens['agente.lsp']?.texto === fab['config.origem.pasta'].pt && origens['agente.lsp']?.contorno === 'solid'
    && origens['agente.agentes_dir']?.texto === fab['config.origem.padrao'].pt && origens['agente.agentes_dir']?.contorno === 'dashed', JSON.stringify(origens));

  // Chave do ambiente: sem editor, com o cadeado e o motivo que o agente deu.
  const amb = await celula(page, 'agente.heartbeat_min', 'valor');
  const ambInfo = await amb.evaluate(td => {
    const t = td.querySelector('.cfg-travado');
    return { editores: td.querySelectorAll('input,select,textarea').length, texto: td.textContent, cadeado: t ? getComputedStyle(t, '::before').content : '' };
  });
  const motivoAmb = VISTA.chaves.find(c => c.chave === 'agente.heartbeat_min').motivo_nao_editavel;
  check('chave do ambiente nao e editavel: nenhum campo, cadeado e o motivo', ambInfo.editores === 0 && ambInfo.texto.includes(motivoAmb) && ambInfo.cadeado.includes('🔒'), JSON.stringify(ambInfo));

  // Segredo: nunca um campo de valor; diz se esta guardado e como se guarda; e o valor
  // que estava no ambiente nao chega ao DOM (o agente nao o manda, e a tela nao inventa).
  const seg = await celula(page, 'api.token', 'valor');
  const segInfo = await seg.evaluate(td => ({ editores: td.querySelectorAll('input,select,textarea').length, rotulo: td.querySelector('.cfg-segredo')?.textContent, cmd: td.querySelector('.cfg-comando')?.textContent }));
  const html = await page.content();
  const cmdEsperado = VISTA.chaves.find(c => c.chave === 'api.token').comando_do_segredo;
  check('segredo: sem campo de valor, mostra «guardado no cofre» e o comando, e nenhum valor no DOM',
    segInfo.editores === 0 && segInfo.rotulo === fab['config.segredo.presente'].pt && segInfo.cmd === cmdEsperado && !html.includes(SEGREDO_NO_AMBIENTE)
    && (await textoCel(page, 'api.token', 'padrao')) === '—', JSON.stringify(segInfo));

  // Editar booleano e enum; o diff aparece na pagina, sem confirm(), antes de gravar.
  let dialogo = false;
  page.on('dialog', d => { dialogo = true; d.dismiss(); });
  await (await celula(page, 'agente.lsp', 'valor')).$('input[type="checkbox"]').then(e => e.click());
  await page.waitForTimeout(300);
  await (await celula(page, 'voz.tts.provedor', 'valor')).$('select').then(e => e.selectOption('elevenlabs'));
  await page.waitForTimeout(300);
  const pendentes = await page.$$eval('#configConteudo tbody tr', trs => trs.filter(t => t.querySelector('td.phx-est-cfg-pendente')).map(t => t.querySelector('td[data-tag="chave"]').textContent));
  check('editar booleano e enum marca as duas linhas como alteradas', pendentes.sort().join() === 'agente.lsp,voz.tts.provedor', pendentes.join());
  await page.click('#configSalvar');
  await page.waitForTimeout(300);
  const diff = await page.$$eval('#configDiff tr[data-chave]', trs => trs.map(t => [...t.children].map(td => td.textContent)));
  check('SALVAR mostra o diff na pagina (de -> para), sem confirm() e sem gravar ainda',
    !dialogo && st.puts.length === 0 && JSON.stringify(diff.sort()) === JSON.stringify([['agente.lsp', 'false', 'true'], ['voz.tts.provedor', 'comando', 'elevenlabs']]), JSON.stringify(diff));
  await page.screenshot({ path: join(OUT, 'ui_config_diff.png') });
  const revAntes = st.vista.revisao;
  await page.click('#configConfirmar');
  await page.waitForTimeout(700);
  const put = st.puts[0];
  const statusDepois = await page.textContent('#configStatus');
  check('gravar: PUT com If-Match da revisao lida e so as chaves mexidas; 200 e a revisao sobe',
    put && Number(put.ifMatch) === revAntes && put.corpo.escopo === 'pasta' && JSON.stringify(put.corpo.valores) === JSON.stringify({ 'agente.lsp': true, 'voz.tts.provedor': 'elevenlabs' })
    && st.vista.revisao === revAntes + 1 && statusDepois.includes(String(revAntes + 1)) && (await textoCel(page, 'voz.tts.provedor', 'origem')) === fab['config.origem.pasta'].pt,
    `${JSON.stringify(put)} -> ${statusDepois}`);

  // 409: alguem grava na frente; a tela avisa, nada se perde, e recarregar mantem o digitado.
  const vozSel = async () => (await celula(page, 'voz.stt.provedor', 'valor')).$('select');
  await (await vozSel()).selectOption('whisper');
  await page.waitForTimeout(300);
  st.forcarConflito = true;
  await page.click('#configSalvar');
  await page.click('#configConfirmar');
  await page.waitForTimeout(600);
  const conflito = await page.$eval('#configConflito', e => ({ visivel: !e.hidden, texto: e.textContent }));
  const aindaDigitado = await (await vozSel()).evaluate(s => s.value);
  check('409: a tela mostra o conflito com a revisao atual e o digitado continua', conflito.visivel && conflito.texto.includes(String(st.vista.revisao)) && aindaDigitado === 'whisper', conflito.texto.slice(0, 120));
  await page.screenshot({ path: join(OUT, 'ui_config_409.png') });
  await page.click('#configRecarregarMantendo');
  await page.waitForTimeout(600);
  const depoisRecarga = await (await vozSel()).evaluate(s => s.value);
  const revRecarregada = await page.textContent('#configStatus');
  check('409 -> RECARREGAR SEM PERDER O DIGITADO: le a revisao nova e o valor digitado continua no campo',
    depoisRecarga === 'whisper' && revRecarregada.includes(String(st.vista.revisao)), `${depoisRecarga} / ${revRecarregada}`);
  if (depoisRecarga !== 'whisper') throw new Error('o digitado se perdeu no recarregar');
  await page.click('#configSalvar');
  await page.click('#configConfirmar');
  await page.waitForTimeout(600);
  const ultimo = st.puts[st.puts.length - 1];
  check('depois do 409, a nova gravacao vai com a revisao nova e leva o digitado (200)',
    Number(ultimo.ifMatch) === st.vista.revisao - 1 && JSON.stringify(ultimo.corpo.valores) === '{"voz.stt.provedor":"whisper"}', JSON.stringify(ultimo));

  // 422: o agente recusa um inteiro quebrado; a celula da chave fica marcada com o motivo.
  const inteiro = await (await celula(page, 'api.tarefas_por_minuto', 'valor')).$('input[type="number"]');
  await inteiro.evaluate(e => { e.value = '2.5'; e.dispatchEvent(new Event('change', { bubbles: true })); });
  await page.waitForTimeout(300);
  await page.click('#configSalvar');
  await page.click('#configConfirmar');
  await page.waitForTimeout(600);
  const cel422 = await celula(page, 'api.tarefas_por_minuto', 'valor');
  const marca = await cel422.evaluate(td => ({ motivo: td.querySelector('.cfg-motivo')?.textContent, erro: td.classList.contains('phx-est-cfg-erro') }));
  const outrasMarcadas = await page.$$eval('#configConteudo td.phx-est-cfg-erro[data-tag="chave"]', t => t.map(x => x.textContent));
  check('422: a celula da chave recusada fica marcada com o motivo do agente (e so ela)', marca.erro && marca.motivo === 'esperado inteiro, veio 2.5' && outrasMarcadas.join() === 'api.tarefas_por_minuto', JSON.stringify(marca));
  await page.screenshot({ path: join(OUT, 'ui_config_422.png') });

  // Voltar ao padrao: por chave, manda null -- inclusive onde o padrao existe (agente.lsp,
  // padrao true): mandar o valor do padrao gravaria a chave no arquivo em vez de tira-la.
  await (await celula(page, 'agente.capacidades', 'acao')).$('button.cfg-volta').then(b => b.click());
  await (await celula(page, 'agente.lsp', 'acao')).$('button.cfg-volta').then(b => b.click());
  await page.waitForTimeout(300);
  await (await celula(page, 'api.tarefas_por_minuto', 'valor')).$('input[type="number"]').then(e => e.evaluate(x => { x.value = '12'; x.dispatchEvent(new Event('change', { bubbles: true })); }));
  await page.waitForTimeout(300);
  await page.click('#configSalvar');
  await page.click('#configConfirmar');
  await page.waitForTimeout(600);
  const pVolta = st.puts[st.puts.length - 1];
  check('VOLTAR AO PADRAO manda null para a chave; a origem volta a padrao', pVolta.corpo.valores['agente.capacidades'] === null && pVolta.corpo.valores['agente.lsp'] === null && pVolta.corpo.valores['api.tarefas_por_minuto'] === 12
    && (await textoCel(page, 'agente.capacidades', 'origem')) === fab['config.origem.padrao'].pt, JSON.stringify(pVolta.corpo));

  // So alterados e busca.
  await page.check('#configSoAlterados');
  await page.waitForTimeout(400);
  const soAlt = await page.$$eval('#configConteudo td[data-tag="chave"]', t => t.length);
  const naoPadrao = st.vista.chaves.filter(c => c.origem !== 'padrao').length;
  await page.uncheck('#configSoAlterados');
  await page.fill('#configBusca', 'heartbeat');
  await page.waitForTimeout(500);
  const busca = await page.$$eval('#configConteudo td[data-tag="chave"]', t => t.map(x => x.textContent));
  await page.fill('#configBusca', '');
  await page.waitForTimeout(500);
  check('«so alterados» mostra so o que nao esta no padrao; a busca acha pela chave', soAlt === naoPadrao && busca.includes('agente.heartbeat_min') && busca.length < 5, `${soAlt}/${naoPadrao} ${busca}`);

  // Botoes so contorno, nas cores da acao.
  const botoes = await page.$$eval('#tela-config .acao', bs => bs.filter(b => b.offsetParent).map(b => ({ c: b.className, fundo: getComputedStyle(b).backgroundColor })));
  // VOLTAR AO PADRAO e rosa (marca): e um desfazer que se reverte, nao um excluir de vez
  // (qualificacao de 01/10/2026, M9). A checagem continua exigindo as tres cores, e agora
  // tambem que nenhum botao desta tela seja vermelho -- nada aqui apaga de vez.
  check('botoes so contorno: salvar amarelo, recarregar azul, voltar ao padrao rosa (marca), nenhum vermelho',
    botoes.every(b => b.fundo === 'rgba(0, 0, 0, 0)') && botoes.some(b => /altera/.test(b.c)) && botoes.some(b => /consulta/.test(b.c)) && botoes.some(b => /\bmarca\b/.test(b.c)) && !botoes.some(b => /exclui/.test(b.c)), JSON.stringify(botoes.slice(0, 4)));

  // Idioma: rotulos mudam e a chave (dado) nao. A DESCRICAO da chave e rotulo (o catalogo a
  // traz em cada idioma; qualificacao, M14): em ingles e a descricao_en da propria chave.
  const ler = () => page.evaluate(() => ({
    cab: [...document.querySelectorAll('#configConteudo thead th[data-tag]:not(.phx-frow-cel) .phx-th-titulo')].map(e => e.firstChild.textContent),
    dado: [...document.querySelectorAll('#configConteudo td[data-tag="chave"]')].map(e => e.textContent).join('|'),
    desc: [...document.querySelectorAll('#configConteudo tbody tr')].filter(tr => tr.querySelector('td[data-tag="descricao"]'))
      .map(tr => [tr.querySelector('td[data-tag="chave"]').textContent, tr.querySelector('td[data-tag="descricao"]').textContent]),
    origem: [...document.querySelectorAll('#configConteudo td[data-tag="origem"]')].map(e => e.textContent),
  }));
  const pt = await ler();
  await page.click('#trocarIdioma');
  await page.waitForTimeout(600);
  const en = await ler();
  await page.click('#trocarIdioma');
  await page.waitForTimeout(600);
  const porChave = new Map(st.vista.chaves.map(c => [c.chave, c]));
  const descOk = (lista, campo) => lista.length > 0 && lista.every(([k, d]) => porChave.get(k)?.[campo] === d);
  check('idioma: cabecalhos, origem e descricao mudam pela chave; a chave (dado) nao',
    en.cab.includes(fab['config.col.chave'].en) && en.origem.includes(fab['config.origem.ambiente'].en) && en.dado === pt.dado && pt.cab.join() !== en.cab.join()
      && descOk(pt.desc, 'descricao') && descOk(en.desc, 'descricao_en'), `${en.cab.slice(0, 3).join(',')} • ${JSON.stringify(en.desc[0])} • ${en.desc.length} descricoes`);
  check('sem erro de JavaScript na pagina', erros.length === 0, erros.join(' | ').slice(0, 300));
  await page.close();

  // Sem token: a tela diz como entrar e mostra o catalogo gerado, so leitura.
  const st2 = criarStub();
  const { page: p2 } = await abrir(browser, st2, { token: '' });
  const semToken = await p2.evaluate(() => ({ aviso: document.getElementById('configStatus').textContent, editores: document.querySelectorAll('#configConteudo .cfg-ed').length, linhas: document.querySelectorAll('#configConteudo td[data-tag="chave"]').length }));
  const catalogo = JSON.parse(readFileSync(join(UI, 'assets/config-catalogo.json'), 'utf8'));
  check('sem token: aviso pela fabrica e o catalogo so leitura (nenhum editor)', semToken.aviso === fab['config.pede_token'].pt && semToken.editores === 0 && semToken.linhas === catalogo.chaves.length, JSON.stringify(semToken));
  await p2.close();

  // Pela ponte do controle remoto a rota e 403 de proposito: a tela diz por que e mostra o
  // catalogo, em vez de um erro cru.
  const { page: p3 } = await abrir(browser, criarStub(), { ponte: true });
  const pelaPonte = await p3.evaluate(() => ({ aviso: document.getElementById('configStatus').textContent, editores: document.querySelectorAll('#configConteudo .cfg-ed').length }));
  check('pela ponte (403): aviso de que o remoto nao administra, catalogo so leitura', pelaPonte.aviso === fab['config.pela_ponte'].pt && pelaPonte.editores === 0, JSON.stringify(pelaPonte));
  await p3.close();
} catch (e) {
  check('roteiro', false, String(e).slice(0, 400));
} finally {
  await browser.close();
}
console.log(`placar: ${checagens.filter(Boolean).length}/${checagens.length}`);
process.exit(checagens.length && checagens.every(Boolean) ? 0 : 1);
