// A tela Fluxos (editor em grafo, SVG a mao) EXERCITADA contra o agente REAL (`phxclaw
// servir`): as rotas /v1/fluxos/* pelo fio (Bearer, confinamento, If-Match, veredito do
// motor), e na tela -- desenhar um fluxo de exemplo em camadas, arrastar um no, salvar e
// conferir o JSON EM DISCO (posicao no campo «ui», assinatura do motor igual), editar um
// parametro, desligar e religar uma ligacao, um ciclo recusado pelo motor (e nao gravado),
// executar o fluxo e executar ate um passo (--ate), com o estado de cada passo por cima do
// grafo e os itens de entrada/saida ao clicar. Nas larguras 1280 e 400, nos dois temas,
// com o contraste medido (>= 4,5:1 no texto) e nenhum text-transform em dado.
//
// Uso: PLAYWRIGHT_BROWSERS_PATH=/opt/pw-browsers node tests/desktop/ui_fluxos.mjs [BINARIO] [--ui DIR]
// Resultado em tests/desktop/out/ui_fluxos.json; capturas ui_fluxos_{1280,400}_{escuro,claro}.png
// e ui_fluxos_execucao.png. RED medido: copia da UI sem o <script src="./assets/fluxos.js">
// derruba o desenho; sem o `.merge(crate::fluxos_tela::rotas())` as rotas dao 404.
import { createRequire } from 'node:module';
import { spawn } from 'node:child_process';
import { mkdtempSync, writeFileSync, readFileSync, mkdirSync, rmSync } from 'node:fs';
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

// O fluxo de exemplo: so passos que rodam sem modelo (ferramentas de arquivo e nos de
// controle), com uma condicao de duas portas, um lote, uma juncao e um fluxo de erro --
// o bastante para o desenho ter cinco camadas, duas portas nomeadas e um no fora do grafo.
// «Blumenau» e dado: tem de aparecer como gravado nos itens da execucao, nunca «BLUMENAU».
const EXEMPLO = {
  nome: 'exemplo-tela',
  passos: [
    { id: 'grava', ferramenta: 'write_file', args: { path: 'dados.json', content: '[{"cidade":"Blumenau","n":3},{"cidade":"Joinville","n":1}]' } },
    { id: 'le', depende: ['grava'], ferramenta: 'read_file', args: { path: 'dados.json' } },
    { id: 'grande', depende: ['le'], se: { caminho: 'n', operador: 'maior', valor: 2 } },
    { id: 'lotes', depende: ['grande:verdadeiro'], lote: 1 },
    { id: 'pequenas', depende: ['grande:falso'], ferramenta: 'write_file', args: { path: 'pequenas.txt', content: 'cidades: {{grande}}' } },
    { id: 'junta', depende: ['lotes', 'pequenas'], juntar: { modo: 'append' } },
    { id: 'avisa', ferramenta: 'write_file', args: { path: 'erro.txt', content: 'falhou: {{erro}}' } },
  ],
  fluxo_de_erro: 'avisa',
};

const D = mkdtempSync(join(tmpdir(), 'phx-ui-fluxos-'));
const PASTA = join(D, 'agente');
const FLUXOS = join(PASTA, 'fluxos');
mkdirSync(FLUXOS, { recursive: true });
const ARQ = join(FLUXOS, 'exemplo.json');
writeFileSync(ARQ, `${JSON.stringify(EXEMPLO, null, 2)}\n`);
writeFileSync(join(FLUXOS, 'quebrado.json'), '{"nome": "quebrado", "passos": []}\n');
writeFileSync(join(D, 'fora.json'), '{"segredo": "fora da pasta"}\n');
const token = 'token-ui-fluxos-' + Date.now();
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
const fio = (metodo, caminho, corpo, extra = {}, t = token) => fetch(`${ORIG}/v1/${caminho}`, {
  method: metodo,
  headers: { ...(t ? { Authorization: `Bearer ${t}` } : {}), 'Content-Type': 'application/json', ...extra },
  body: corpo === undefined ? undefined : JSON.stringify(corpo),
});
const lerDisco = () => JSON.parse(readFileSync(ARQ, 'utf8'));

// Contraste WCAG entre duas cores rgb()/rgba() ja resolvidas pelo navegador.
const CONTRASTE = `(() => {
  const rgb = s => { const m = String(s).match(/[\\d.]+/g) || [0, 0, 0]; return m.slice(0, 3).map(Number); };
  const lum = c => { const [r, g, b] = c.map(v => { v /= 255; return v <= 0.03928 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4; }); return 0.2126 * r + 0.7152 * g + 0.0722 * b; };
  const razao = (a, b) => { const [x, y] = [lum(rgb(a)), lum(rgb(b))].sort((p, q) => q - p); return +((x + 0.05) / (y + 0.05)).toFixed(2); };
  // Fundo opaco mais proximo de um elemento HTML.
  const fundo = e => { for (let n = e; n; n = n.parentElement) { const c = getComputedStyle(n).backgroundColor; if (c && !/rgba\\(.*, 0\\)$/.test(c) && c !== 'transparent') return c; } return getComputedStyle(document.body).backgroundColor; };
  const medir = [];
  const caixa = document.querySelector('#fluxosSvg .no .no-caixa');
  const fundoNo = caixa ? getComputedStyle(caixa).fill : null;
  for (const [nome, sel] of [['no: tipo', '.no-tipo'], ['no: id', '.no-id'], ['no: resumo', '.no-resumo']]) {
    const t = document.querySelector('#fluxosSvg ' + sel);
    if (t) medir.push({ nome, razao: razao(getComputedStyle(t).fill, fundoNo) });
  }
  const aba = document.querySelector('#fluxosSvg .no-estado');
  if (aba) medir.push({ nome: 'no: aba de estado', razao: razao(getComputedStyle(aba).fill, getComputedStyle(document.querySelector('#fluxosSvg .no-estado-caixa')).fill) });
  const porta = document.querySelector('#fluxosSvg .aresta-porta');
  if (porta) medir.push({ nome: 'aresta: porta (sobre o fundo do quadro)', razao: razao(getComputedStyle(porta).fill, fundo(document.getElementById('fluxosPalco') || document.querySelector('.fluxos-palco'))) });
  for (const sel of ['#fluxosValidar', '#fluxosSalvar', '#fluxosRodar', '#fluxosNome', '.fluxo-item-nome', '.fluxo-item-arq', '#fluxosVeredito', '.fp-id', '.fp-itens', '.fluxos-legenda li span']) {
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

  // ---------------------------------------------------------------- a rota pelo fio
  const semToken = await fio('GET', 'fluxos', undefined, {}, null);
  check('rota: sem Bearer e 401', semToken.status === 401, semToken.status);
  const listaFio = await (await fio('GET', 'fluxos')).json();
  check('rota: GET /v1/fluxos lista o valido e o invalido (com o motivo do motor)',
    listaFio.fluxos?.length === 1 && listaFio.fluxos[0].arquivo === 'exemplo.json' && listaFio.fluxos[0].passos === 7
    && listaFio.invalidos?.length === 1 && listaFio.invalidos[0].arquivo === 'quebrado.json' && /passos/.test(listaFio.invalidos[0].erro), JSON.stringify(listaFio));
  const fora = await fio('GET', `fluxos/arquivo?nome=${encodeURIComponent('../fora.json')}`);
  const foraTxt = await fora.text();
  check('rota: ../ fora da pasta e 404 e nao vaza o texto', fora.status === 404 && !foraTxt.includes('fora da pasta'), `${fora.status} ${foraTxt}`);
  const lido = await (await fio('GET', 'fluxos/arquivo?nome=exemplo.json')).json();
  check('rota: GET arquivo devolve texto, revisao e o veredito do motor', lido.valido === true && lido.passos === 7 && /^[0-9a-f]{64}$/.test(lido.revisao) && /^[0-9a-f]{64}$/.test(lido.assinatura), JSON.stringify({ ...lido, texto: undefined }));
  const semIfMatch = await fio('PUT', 'fluxos/arquivo?nome=exemplo.json', { texto: lido.texto });
  const ifMatchVelho = await fio('PUT', 'fluxos/arquivo?nome=exemplo.json', { texto: lido.texto }, { 'If-Match': '0'.repeat(64) });
  const ciclo = JSON.parse(lido.texto);
  ciclo.passos[0].depende = ['junta'];
  const putCiclo = await fio('PUT', 'fluxos/arquivo?nome=exemplo.json', { texto: JSON.stringify(ciclo) }, { 'If-Match': lido.revisao });
  const putCicloV = await putCiclo.json();
  const novoArq = await fio('PUT', 'fluxos/arquivo?nome=novo.json', { texto: lido.texto }, { 'If-Match': lido.revisao });
  check('rota: PUT sem If-Match 428, revisao velha 409, ciclo 422 com o motivo do motor, arquivo novo 404',
    semIfMatch.status === 428 && ifMatchVelho.status === 409 && putCiclo.status === 422 && putCicloV.valido === false && /ciclo|cycle/i.test(putCicloV.erro) && novoArq.status === 404,
    `${semIfMatch.status} ${ifMatchVelho.status} ${putCiclo.status} ${putCicloV.erro} ${novoArq.status}`);
  check('rota: nada disso tocou o disco', readFileSync(ARQ, 'utf8') === lido.texto);

  // ---------------------------------------------------------------- a tela
  const browser = await chromium.launch();
  const erros = [];
  // O agente real manda a CSP (pwa.rs): o console de cada contexto e vigiado.
  const vigias = [];
  async function abrirTela(largura, temaEscolhido) {
    const ctx = await browser.newContext({ viewport: { width: largura, height: largura < 640 ? 860 : 900 }, locale: 'pt-BR', deviceScaleFactor: 1, hasTouch: largura < 640 });
    vigias.push(await vigiarCsp(ctx));
    await ctx.addInitScript(([t, tm]) => { try { localStorage.setItem('phxclaw.token', t); localStorage.setItem('phxclaw.tema', tm); } catch {} }, [token, temaEscolhido]);
    const page = await ctx.newPage();
    page.on('pageerror', e => erros.push(`${largura}/${temaEscolhido}: ${e}`));
    page.on('dialog', d => d.accept());
    await page.goto(`${ORIG}/?tela=fluxos`);
    await page.waitForSelector('#tela-fluxos:not([hidden])');
    await page.waitForSelector('.fluxo-item[data-arquivo="exemplo.json"]');
    return { ctx, page };
  }
  const grafo = page => page.evaluate(() => ({
    nos: [...document.querySelectorAll('#fluxosSvg .no')].map(g => ({ id: g.dataset.id, t: g.getAttribute('transform'), cls: g.getAttribute('class') })),
    arestas: [...document.querySelectorAll('#fluxosSvg .aresta')].map(g => `${g.dataset.dep}->${g.dataset.para}`),
    portas: [...document.querySelectorAll('#fluxosSvg .aresta-porta')].map(t => t.textContent),
    veredito: document.getElementById('fluxosVeredito').textContent,
    valido: document.getElementById('fluxosVeredito').dataset.valido,
    sujo: !document.getElementById('fluxosSujo').hidden,
  }));
  const xy = t => (t || '').match(/-?[\d.]+/g)?.map(Number) || [];

  // ---- 1280, escuro: o percurso inteiro de edicao
  {
    const { ctx, page } = await abrirTela(1280, 'escuro');
    const item = await page.$eval('.fluxo-item[data-arquivo="exemplo.json"]', b => ({ nome: b.querySelector('.fluxo-item-nome').textContent, tt: getComputedStyle(b.querySelector('.fluxo-item-nome')).textTransform }));
    const invalido = await page.$eval('.fluxo-item[data-arquivo="quebrado.json"]', b => b.textContent);
    check('lista: o fluxo pelo nome gravado (sem text-transform) e o invalido marcado com o motivo', item.nome === 'exemplo-tela' && item.tt === 'none' && /INVÁLIDO/.test(invalido) && /passos/.test(invalido), JSON.stringify({ item, invalido }));
    await page.click('.fluxo-item[data-arquivo="exemplo.json"]');
    await page.waitForSelector('#fluxosSvg .no');
    await page.waitForTimeout(400);
    const g0 = await grafo(page);
    const camada = id => xy(g0.nos.find(n => n.id === id)?.t)[0];
    check('desenho: 7 nos e 6 ligacoes, com as portas nomeadas como gravadas', g0.nos.length === 7 && g0.arestas.length === 6 && g0.portas.sort().join(',') === 'falso,verdadeiro', JSON.stringify(g0));
    check('desenho: camadas pela ordem topologica (grava < le < grande < lotes = pequenas < junta)',
      camada('grava') < camada('le') && camada('le') < camada('grande') && camada('grande') < camada('lotes') && camada('lotes') === camada('pequenas') && camada('pequenas') < camada('junta'),
      g0.nos.map(n => `${n.id}@${n.t}`).join(' '));
    // As duas portas do «se» saem do mesmo ponto: os nomes nao podem se sobrepor (sobrepunham:
    // «falso» escrito em cima de «verdadeiro», visto na captura).
    const caixasPorta = await page.$$eval('#fluxosSvg .aresta-porta', ts => ts.map(t => { const b = t.getBoundingClientRect(); return [b.left, b.top, b.right, b.bottom]; }));
    const cruzam = caixasPorta.some((a, i) => caixasPorta.some((b, j) => j > i && a[0] < b[2] && b[0] < a[2] && a[1] < b[3] && b[1] < a[3]));
    // ...nem ficam debaixo de um no (o vao entre camadas tem de caber o nome).
    const sobNo = await page.$$eval('#fluxosSvg .aresta-porta', ts => { const nos = [...document.querySelectorAll('#fluxosSvg .no-caixa, #fluxosSvg .no-estado-caixa')].map(r => r.getBoundingClientRect()); return ts.some(t => { const a = t.getBoundingClientRect(); return nos.some(b => a.left < b.right && b.left < a.right && a.top < b.bottom && b.top < a.bottom); }); });
    check('desenho: os nomes das portas nao se sobrepoem nem ficam sob um no', caixasPorta.length === 2 && !cruzam && !sobNo, JSON.stringify(caixasPorta));
    const erroNo = g0.nos.find(n => n.id === 'avisa');
    check('desenho: o fluxo de erro fica fora do grafo, embaixo e sem ligacao', /no-erro/.test(erroNo?.cls) && xy(erroNo.t)[1] > Math.max(...g0.nos.filter(n => n.id !== 'avisa').map(n => xy(n.t)[1])) && !g0.arestas.some(a => a.includes('avisa')), JSON.stringify(erroNo));
    check('veredito do motor ao abrir: valido, 7 passos', g0.valido === '1' && /7 passos/.test(g0.veredito), g0.veredito);
    // Legivel ao abrir: o menor texto do no, na TELA (fonte x zoom), nao fica abaixo de 12 px.
    const menor = await page.evaluate(() => Math.min(...['.no-tipo', '.no-id', '.no-resumo'].map(c => parseFloat(getComputedStyle(document.querySelector('#fluxosSvg ' + c)).fontSize) * window.fluxosTela.k)));
    check('legivel ao abrir: o menor texto do no tem >= 12 px na tela', menor >= 12, menor);
    await page.screenshot({ path: join(OUT, 'ui_fluxos_1280_escuro.png') });
    capturas.push('ui_fluxos_1280_escuro.png');

    // Zoom: + aumenta, AJUSTAR volta a caber.
    const k0 = await page.evaluate(() => window.fluxosTela.k);
    await page.click('#fluxosMais');
    const k1 = await page.evaluate(() => window.fluxosTela.k);
    await page.click('#fluxosAjustar');
    const k2 = await page.evaluate(() => window.fluxosTela.k);
    const cabe = await page.evaluate(() => { const q = document.getElementById('fluxosQuadro'); const nos = [...document.querySelectorAll('#fluxosSvg .no-caixa')].map(r => r.getBoundingClientRect()); const b = q.getBoundingClientRect(); return nos.every(r => r.left >= b.left - 1 && r.right <= b.right + 1 && r.top >= b.top - 1 && r.bottom <= b.bottom + 1); });
    check('zoom: abre em 100%, + aumenta 25% e AJUSTAR poe todos os nos dentro do quadro', k0 === 1 && Math.abs(k1 / k0 - 1.25) < 0.01 && k2 < 1 && cabe, `${k0} ${k1} ${k2} cabe=${cabe}`);

    // Arrastar o no «junta» 90 px para a direita e 60 para baixo (na tela), salvar, conferir o disco.
    const caixaNo = await page.$eval('#fluxosSvg .no[data-id="junta"] .no-caixa', r => { const b = r.getBoundingClientRect(); return { x: b.left + b.width / 2, y: b.top + b.height / 2 }; });
    const antes = await page.evaluate(() => ({ ...window.fluxosTela.pos.get('junta') }));
    const k = await page.evaluate(() => window.fluxosTela.k);
    await page.mouse.move(caixaNo.x, caixaNo.y);
    await page.mouse.down();
    for (let i = 1; i <= 6; i++) await page.mouse.move(caixaNo.x + 15 * i, caixaNo.y + 10 * i);
    await page.mouse.up();
    await page.waitForTimeout(200);
    const depois = await page.evaluate(() => ({ ...window.fluxosTela.pos.get('junta') }));
    const g1 = await grafo(page);
    check('arrastar: o no anda o que o mouse andou (dividido pelo zoom) e o fluxo fica ALTERADO',
      Math.abs(depois.x - antes.x - 90 / k) <= 2 && Math.abs(depois.y - antes.y - 60 / k) <= 2 && g1.sujo, JSON.stringify({ antes, depois, k }));
    await page.click('#fluxosSalvar');
    await page.waitForFunction(() => document.getElementById('fluxosSujo').hidden);
    const disco1 = lerDisco();
    check('salvar: a posicao vai para ui.posicoes no JSON em disco, e os passos ficam iguais',
      disco1.ui?.posicoes?.junta?.x === depois.x && disco1.ui.posicoes.junta.y === depois.y && JSON.stringify(disco1.passos) === JSON.stringify(EXEMPLO.passos),
      JSON.stringify(disco1.ui));
    const relido = await (await fio('GET', 'fluxos/arquivo?nome=exemplo.json')).json();
    check('o motor ignora o campo ui: continua valido e a ASSINATURA e a mesma de antes do arrasto', relido.valido && relido.assinatura === lido.assinatura, `${lido.assinatura} -> ${relido.assinatura}`);
    // Reabrir desenha onde foi salvo (a posicao vem do arquivo, nao da memoria da aba).
    await page.reload();
    await page.waitForSelector('.fluxo-item[data-arquivo="exemplo.json"]');
    await page.click('.fluxo-item[data-arquivo="exemplo.json"]');
    await page.waitForSelector('#fluxosSvg .no[data-id="junta"]');
    const reaberto = await page.evaluate(() => ({ ...window.fluxosTela.pos.get('junta') }));
    check('reabrir: o no volta na posicao salva', reaberto.x === depois.x && reaberto.y === depois.y, JSON.stringify(reaberto));

    // Editar um parametro pelo painel lateral (o conteudo do «pequenas»), aplicar e salvar.
    await page.click('#fluxosSvg .no[data-id="pequenas"] .no-caixa');
    await page.waitForSelector('.fp-json');
    const params = JSON.parse(await page.inputValue('.fp-json'));
    params.args.content = 'pequenas: {{grande}}';
    await page.fill('.fp-json', JSON.stringify(params, null, 2));
    await page.click('.fp-bloco .acao.altera');
    await page.waitForTimeout(500);
    await page.click('#fluxosSalvar');
    await page.waitForFunction(() => document.getElementById('fluxosSujo').hidden);
    const disco2 = lerDisco();
    const pq = disco2.passos.find(p => p.id === 'pequenas');
    check('parametro: o painel troca o args do passo, mantem id e depende, e o disco recebe', pq.args.content === 'pequenas: {{grande}}' && pq.depende.join() === 'grande:falso' && disco2.passos.length === 7, JSON.stringify(pq));
    // JSON quebrado no painel nao derruba nada: diz o erro e nao aplica.
    await page.fill('.fp-json', '{ quebrado');
    await page.click('.fp-bloco .acao.altera');
    const msg = await page.$eval('.fp-msg', e => ({ t: e.textContent, erro: e.dataset.erro }));
    check('parametro: JSON invalido no painel e recusado com o motivo, sem marcar ALTERADO', msg.erro === '1' && /JSON inválido/.test(msg.t) && !(await grafo(page)).sujo, JSON.stringify(msg));

    // Desligar uma ligacao (pelo teclado, na propria aresta) e religar pelo painel.
    await page.focus('#fluxosSvg .aresta[data-para="junta"][data-dep="pequenas"]');
    await page.keyboard.press('Enter');
    await page.click('.fluxos-painel .acao.exclui');
    await page.waitForTimeout(500);
    const g2 = await grafo(page);
    check('desligar: a ligacao some do desenho e o motor ainda aceita (5 ligacoes)', g2.arestas.length === 5 && !g2.arestas.includes('pequenas->junta') && g2.valido === '1' && g2.sujo, JSON.stringify(g2.arestas));
    await page.click('#fluxosSvg .no[data-id="junta"] .no-caixa');
    await page.selectOption('.fp-ligar select >> nth=0', 'pequenas');
    await page.click('.fp-ligar .acao.inclui');
    await page.waitForTimeout(500);
    const g3 = await grafo(page);
    check('ligar: a ligacao volta (6) pelo painel', g3.arestas.length === 6 && g3.arestas.includes('pequenas->junta') && g3.valido === '1', JSON.stringify(g3.arestas));
    // Ligar com porta: o «se» oferece verdadeiro/falso.
    await page.click('#fluxosSvg .no[data-id="junta"] .no-caixa');
    await page.selectOption('.fp-ligar select >> nth=0', 'grande');
    const portasOferecidas = await page.$$eval('.fp-ligar select >> nth=1', s => [...s[0].options].map(o => o.value));
    check('ligar: origem «se» oferece as portas do motor (principal, verdadeiro, falso)', portasOferecidas.join(',') === ',verdadeiro,falso', portasOferecidas.join(','));
    // Um ciclo: o motor recusa, a tela diz, e SALVAR nao grava.
    await page.click('#fluxosSvg .no[data-id="grava"] .no-caixa');
    await page.selectOption('.fp-ligar select >> nth=0', 'junta');
    await page.click('.fp-ligar .acao.inclui');
    await page.waitForFunction(() => document.getElementById('fluxosVeredito').dataset.valido === '0', null, { timeout: 4000 }).catch(() => {});
    const g4 = await grafo(page);
    const textoAntes = readFileSync(ARQ, 'utf8');
    await page.click('#fluxosSalvar');
    await page.waitForTimeout(500);
    const rodarDesligado = await page.$eval('#fluxosRodar', b => b.disabled);
    check('ciclo: o motor recusa (veredito vermelho com o motivo), SALVAR nao grava e EXECUTAR fica desligado',
      g4.valido === '0' && /ciclo|cycle/i.test(g4.veredito) && readFileSync(ARQ, 'utf8') === textoAntes && rodarDesligado, g4.veredito);
    await page.screenshot({ path: join(OUT, 'ui_fluxos_ciclo.png') });
    capturas.push('ui_fluxos_ciclo.png');
    // Desfaz o ciclo e salva de novo.
    await page.click('#fluxosSvg .no[data-id="grava"] .no-caixa');
    await page.click('.fp-deps .acao.exclui');
    await page.waitForFunction(() => document.getElementById('fluxosVeredito').dataset.valido === '1');
    await page.click('#fluxosSalvar');
    await page.waitForFunction(() => document.getElementById('fluxosSujo').hidden);

    // Passo novo pelo painel (+ PASSO): tipo lote, id «extra», ligado a saida do «junta»; o
    // motor aceita, o disco recebe. Depois EXCLUIR PASSO tira o passo, a ligacao e a posicao.
    const passosAntes = JSON.stringify(lerDisco().passos);
    await page.click('#fluxosNovoPasso');
    await page.selectOption('.fp-sel', 'lote');
    await page.fill('.fp-campo', 'extra');
    await page.click('.fp-bloco .acao.inclui');
    await page.waitForSelector('#fluxosSvg .no[data-id="extra"]');
    await page.selectOption('.fp-ligar select >> nth=0', 'junta');
    await page.click('.fp-ligar .acao.inclui');
    await page.waitForFunction(() => document.getElementById('fluxosVeredito').textContent.includes('8 passos'), null, { timeout: 4000 }).catch(() => {});
    const g7 = await grafo(page);
    await page.click('#fluxosSalvar');
    await page.waitForFunction(() => document.getElementById('fluxosSujo').hidden, null, { timeout: 4000 }).catch(() => {});
    const disco3 = lerDisco();
    const extra = disco3.passos.find(p => p.id === 'extra');
    check('+ PASSO: o passo novo (lote, ligado ao junta) aparece, o motor aceita e o disco recebe com a posicao',
      g7.nos.length === 8 && g7.arestas.includes('junta->extra') && g7.valido === '1' && extra?.lote === 10 && extra.depende?.join() === 'junta' && Number.isFinite(disco3.ui?.posicoes?.extra?.x),
      JSON.stringify({ extra, pos: disco3.ui?.posicoes?.extra, veredito: g7.veredito }));
    await page.click('#fluxosSvg .no[data-id="extra"] .no-caixa');
    await page.click('.fp-bloco .acao.exclui >> text=EXCLUIR PASSO');
    await page.waitForTimeout(400);
    await page.click('#fluxosSalvar');
    await page.waitForFunction(() => document.getElementById('fluxosSujo').hidden, null, { timeout: 4000 }).catch(() => {});
    const disco4 = lerDisco();
    check('EXCLUIR PASSO: some do desenho e do disco, com a ligacao e a posicao (os outros passos ficam como estavam)',
      (await grafo(page)).nos.length === 7 && JSON.stringify(disco4.passos) === passosAntes && !('extra' in (disco4.ui?.posicoes || {})), JSON.stringify(disco4.ui));

    // Executar o fluxo inteiro: o estado de cada passo por cima do grafo.
    await page.click('#fluxosRodar');
    await page.waitForFunction(() => /concluída|falhou/i.test(document.getElementById('fluxosStatus').textContent) && document.querySelectorAll('#fluxosSvg .no[class*="est-"]').length >= 6, null, { timeout: 30000 }).catch(() => {});
    const g5 = await grafo(page);
    const estados = Object.fromEntries(g5.nos.map(n => [n.id, (n.cls.match(/est-(\w+)/) || [])[1] || null]));
    check('executar: os seis passos do grafo saem OK por cima do desenho (contorno da cor de estado + aba com o rotulo)',
      ['grava', 'le', 'grande', 'lotes', 'pequenas', 'junta'].every(i => estados[i] === 'ok'), JSON.stringify(estados) + ' ' + await page.textContent('#fluxosStatus'));
    const aba = await page.$$eval('#fluxosSvg .no[data-id="le"] .no-estado', t => t.map(x => x.textContent));
    const legenda = await page.$$eval('#fluxosLegenda li', l => l.length);
    check('executar: a aba diz o estado em texto (nao so cor) e a legenda aparece com os 7 estados', aba[0] === 'OK' && legenda === 7, `${aba} ${legenda}`);
    // Clique no passo: entrada e saida da execucao, com o dado como gravado.
    await page.click('#fluxosSvg .no[data-id="grande"] .no-caixa');
    await page.waitForSelector('.fp-itens');
    const itens = await page.$$eval('.fp-itens', ps => ps.map(p => ({ t: p.textContent, tt: getComputedStyle(p).textTransform })));
    check('itens: o clique mostra a ENTRADA (saida do «le») e a SAIDA e as PORTAS do «se»',
      itens.length >= 3 && /"le"/.test(itens[0].t) && /Blumenau/.test(itens[0].t) && /verdadeiro/.test(itens[2]?.t || '') && /Joinville/.test(itens[2]?.t || ''), JSON.stringify(itens.map(i => i.t.slice(0, 120))));
    check('dado nunca: «Blumenau» aparece como gravado (sem BLUMENAU, sem text-transform)', itens.every(i => i.tt === 'none') && !itens.some(i => /BLUMENAU/.test(i.t)), itens.map(i => i.tt).join());
    await page.screenshot({ path: join(OUT, 'ui_fluxos_execucao.png') });
    capturas.push('ui_fluxos_execucao.png');
    medidas.contraste_1280_escuro = await page.evaluate(CONTRASTE);

    // Executar ATE o «grande»: o corte --ate; os de fora saem FORA DO CORTE.
    await page.click('#fluxosSvg .no[data-id="grande"] .no-caixa');
    await page.click('.fp-bloco .acao.inclui >> text=EXECUTAR ATÉ AQUI');
    await page.waitForFunction(() => /até o passo grande/.test(document.getElementById('fluxosExecucao').textContent) && !/pending|running/.test(document.getElementById('fluxosStatus').textContent) && document.querySelector('#fluxosSvg .no[data-id="junta"]')?.getAttribute('class').includes('est-nao_pedido'), null, { timeout: 30000 }).catch(() => {});
    const g6 = await grafo(page);
    const est6 = Object.fromEntries(g6.nos.map(n => [n.id, (n.cls.match(/est-(\w+)/) || [])[1] || null]));
    check('executar ate aqui: grava/le/grande OK e lotes/pequenas/junta FORA DO CORTE (o --ate do motor)',
      ['grava', 'le', 'grande'].every(i => est6[i] === 'ok') && ['lotes', 'pequenas', 'junta'].every(i => est6[i] === 'nao_pedido'), JSON.stringify(est6) + ' ' + await page.textContent('#fluxosExecucao'));
    // O corte fica a direita do quadro a 100%: rola ate ele para a captura mostrar os de fora.
    await page.evaluate(() => { document.getElementById('fluxosQuadro').scrollLeft = 520; });
    await page.waitForTimeout(150);
    await page.screenshot({ path: join(OUT, 'ui_fluxos_ate.png') });
    capturas.push('ui_fluxos_ate.png');
    await ctx.close();
  }

  // ---- as quatro combinacoes de largura e tema: desenho, contraste, rolagem
  for (const [largura, tm] of [[1280, 'claro'], [400, 'escuro'], [400, 'claro']]) {
    const { ctx, page } = await abrirTela(largura, tm);
    await page.click('.fluxo-item[data-arquivo="exemplo.json"]');
    await page.waitForSelector('#fluxosSvg .no');
    await page.waitForFunction(() => document.querySelectorAll('#fluxosSvg .no[class*="est-"]').length > 0, null, { timeout: 8000 }).catch(() => {});
    await page.click('#fluxosSvg .no[data-id="le"] .no-caixa');
    await page.waitForTimeout(300);
    const tema = await page.evaluate(() => document.documentElement.dataset.tema);
    const m = await page.evaluate(() => {
      const q = document.getElementById('fluxosQuadro'), l = document.getElementById('fluxosLista'), ed = document.getElementById('fluxosEditor');
      const p = document.getElementById('fluxosPainel');
      return {
        paginaLarga: document.documentElement.scrollWidth > innerWidth + 1,
        telaLarga: document.getElementById('tela-fluxos').scrollWidth > document.getElementById('tela-fluxos').clientWidth + 1,
        quadroRola: getComputedStyle(q).overflow, quadroW: q.clientWidth, svgW: document.getElementById('fluxosSvg').getBoundingClientRect().width,
        listaAcima: l.getBoundingClientRect().bottom <= ed.getBoundingClientRect().top + 1,
        painelAbaixo: p.getBoundingClientRect().top >= q.getBoundingClientRect().bottom - 1,
        mini: getComputedStyle(document.getElementById('fluxosMinimapa')).display,
        nos: document.querySelectorAll('#fluxosSvg .no').length,
      };
    });
    if (largura === 400) {
      check(`400/${tm}: a lista vira coluna EM CIMA, o painel vem embaixo e o minimapa sai`, m.listaAcima && m.painelAbaixo && m.mini === 'none', JSON.stringify(m));
      check(`400/${tm}: nada passa da largura da pagina; o grafo rola no proprio quadro`, !m.paginaLarga && !m.telaLarga && m.quadroRola === 'auto' && m.nos === 7, JSON.stringify(m));
      // O zoom aumenta e o quadro passa a rolar sozinho, sem alargar a pagina.
      await page.click('#fluxosMais');
      await page.click('#fluxosMais');
      const rola = await page.evaluate(() => { const q = document.getElementById('fluxosQuadro'); q.scrollLeft = 120; return { sw: q.scrollWidth, cw: q.clientWidth, sl: q.scrollLeft, pagina: document.documentElement.scrollWidth > innerWidth + 1 }; });
      check(`400/${tm}: com zoom o quadro rola por dentro (scrollLeft anda) e a pagina nao alarga`, rola.sw > rola.cw && rola.sl > 0 && !rola.pagina, JSON.stringify(rola));
      // Reabre (volta a 100%) para a captura mostrar o que a pessoa ve ao abrir.
      await page.click('.fluxo-item[data-arquivo="exemplo.json"]');
      await page.waitForTimeout(400);
      await page.click('#fluxosSvg .no[data-id="le"] .no-caixa');
      await page.evaluate(() => document.getElementById('fluxosEditor').scrollIntoView());
      await page.waitForTimeout(200);
    } else {
      check(`${largura}/${tm}: lista em faixa acima do editor (abaixo de 1500 px), minimapa visivel e o grafo inteiro desenhado`, m.listaAcima && m.mini !== 'none' && m.nos === 7 && !m.paginaLarga, JSON.stringify(m));
    }
    check(`${largura}/${tm}: o tema pedido esta aplicado`, tema === tm, tema);
    const c = await page.evaluate(CONTRASTE);
    medidas[`contraste_${largura}_${tm}`] = c;
    const ruins = c.filter(x => x.razao < 4.5);
    check(`${largura}/${tm}: contraste >= 4,5:1 em todo texto medido (${c.length} amostras)`, c.length >= 8 && !ruins.length, JSON.stringify(ruins.length ? ruins : c.map(x => `${x.nome}=${x.razao}`)));
    const nome = `ui_fluxos_${largura}_${tm}.png`;
    await page.screenshot({ path: join(OUT, nome) });
    capturas.push(nome);
    if (largura === 400) {
      // O painel do passo, embaixo do quadro no celular (a tela rola por dentro do <main>).
      await page.evaluate(() => document.getElementById('fluxosPainel').scrollIntoView());
      await page.waitForTimeout(200);
      const nomeP = `ui_fluxos_400_${tm}_painel.png`;
      await page.screenshot({ path: join(OUT, nomeP) });
      capturas.push(nomeP);
    }
    await ctx.close();
  }
  check('contraste 1280/escuro (com execucao) >= 4,5:1', (medidas.contraste_1280_escuro || []).every(x => x.razao >= 4.5) && medidas.contraste_1280_escuro?.length >= 8, JSON.stringify(medidas.contraste_1280_escuro));
  check('nenhum erro de pagina', !erros.length, erros.join(' | '));
  const csp = vigias.flatMap(v => v.vistas);
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
writeFileSync(join(OUT, 'ui_fluxos.json'), JSON.stringify({ roteiro: 'tests/desktop/ui_fluxos.mjs', medido_em: new Date().toISOString(), placar: `${passou}/${checagens.length}`, ok: passou === checagens.length && checagens.length > 0 && !falhou, checagens, medidas, capturas }, null, 1));
process.exit(checagens.length && passou === checagens.length && !falhou ? 0 : 1);
