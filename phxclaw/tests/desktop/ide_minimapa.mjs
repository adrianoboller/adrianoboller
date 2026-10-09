// O minimapa do IDE web (SP000032 R5, hipotese (b) do pesquisador) com o agente REAL
// (`phxclaw servir`) e o Helix REAL no PTY: o painel desenha o arquivo lido pelo
// /v1/ide/arquivo, a faixa visivel sai da calha do Helix na grade, o clique manda `:goto N`
// e o Helix vai la (a linha de estado diz), a faixa acompanha o cursor, e com
// `line-number = relative` o painel cai para «so cursor» em vez de desenhar faixa falsa.
// Captura nos dois temas. O que NAO prova: texto nao salvo no mapa -- e o limite declarado.
//
// Uso: PLAYWRIGHT_BROWSERS_PATH=/opt/pw-browsers node tests/desktop/ide_minimapa.mjs [BINARIO] [--ui DIR]
// Resultado em tests/desktop/out/ide_minimapa.json; capturas ide_minimapa_{escuro,claro,relativo}.png.
// Exige hx (/opt/helix); sem ele sai 1 dizendo que falta. Nao exige rust-analyzer.
import { createRequire } from 'node:module';
import { spawn } from 'node:child_process';
import { mkdtempSync, writeFileSync, mkdirSync, rmSync, existsSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

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
  checagens.push({ nome, ok: !!ok, detalhe: String(detalhe).slice(0, 300) });
  console.log(`${ok ? 'ok   ' : 'FALHA '}${nome}${detalhe ? ` :: ${String(detalhe).slice(0, 300)}` : ''}`);
}
const esperar = ms => new Promise(r => setTimeout(r, ms));
if (!existsSync('/opt/helix/hx') && !process.env.PHXCLAW_HX) { console.log('FALHA hx ausente (tools/instalar_helix.sh)'); process.exit(1); }

const D = mkdtempSync(join(tmpdir(), 'phx-ide-minimapa-'));
const PROJ = join(D, 'projeto');
mkdirSync(join(PROJ, 'src'), { recursive: true });
// 240 linhas: mais que a altura do terminal, para a faixa ser uma parte do mapa e o clique
// no fim do mapa obrigar o Helix a rolar.
const TOTAL = 240;
const corpo = Array.from({ length: TOTAL }, (_, i) => (i % 12 === 0 ? `fn f${i}() {` : i % 12 === 11 ? '}' : `    let v${i} = ${i} * 2; // linha ${i + 1}`)).join('\n') + '\n';
writeFileSync(join(PROJ, 'src/longo.rs'), corpo);
writeFileSync(join(D, 'segredo.txt'), 'fora do projeto\n');
const token = 'token-ide-minimapa-' + Date.now();
const PASTA = join(D, 'agente');
mkdirSync(PASTA, { recursive: true });
writeFileSync(join(PASTA, 'api.token'), token);
const porta = 19700 + Math.floor(Math.random() * 1000);
const log = { agente: '' };
const agente = spawn(BIN, ['servir', '--porta', String(porta), '--pasta', PASTA], {
  env: { PATH: process.env.PATH, HOME: D, PHXCLAW_PROJETO: PROJ, ...(UI_DIR ? { PHXCLAW_UI_DIR: UI_DIR } : {}), PHXCLAW_MODELO: 'ollama:falso', OLLAMA_HOST: 'http://127.0.0.1:1', PHXCLAW_API_TOKEN: token },
  stdio: ['ignore', 'pipe', 'pipe'],
});
agente.stdout.on('data', b => { log.agente += b; });
agente.stderr.on('data', b => { log.agente += b; });
const capturas = [];
let falhou = null;
try {
  let vivo = false;
  for (let i = 0; i < 100 && !vivo; i++) {
    await esperar(200);
    try { vivo = (await fetch(`http://127.0.0.1:${porta}/health`)).ok; } catch {}
  }
  check('o agente real sobe e serve a UI', vivo, log.agente.split('\n').slice(-3).join(' | '));
  const ORIG = `http://127.0.0.1:${porta}`;

  // A rota pelo fio, antes da tela: Bearer, confinamento e o texto do disco.
  const get = (q, t = token) => fetch(`${ORIG}/v1/ide/arquivo?caminho=${encodeURIComponent(q)}`, { headers: t ? { Authorization: `Bearer ${t}` } : {} });
  const semToken = await get('src/longo.rs', null);
  const fora = await get('../segredo.txt');
  const foraTxt = await fora.text();
  const dentro = await get('src/longo.rs');
  const dentroV = await dentro.json().catch(() => ({}));
  check('rota: sem Bearer e 401', semToken.status === 401, semToken.status);
  check('rota: ../ fora do projeto e 403 e nao vaza o texto', fora.status === 403 && !foraTxt.includes('fora do projeto'), `${fora.status} ${foraTxt}`);
  check(`rota: dentro do projeto devolve o arquivo (${TOTAL} linhas)`, dentro.status === 200 && dentroV.linhas === TOTAL, `${dentro.status} ${dentroV.linhas}`);

  const browser = await chromium.launch();
  const ctx = await browser.newContext({ viewport: { width: 1366, height: 768 }, locale: 'pt-BR', colorScheme: 'dark' });
  await ctx.addInitScript(t => { try { localStorage.setItem('phxclaw.token', t); localStorage.removeItem('phxclaw.tema'); } catch {} }, token);
  const page = await ctx.newPage();
  const erros = [];
  page.on('pageerror', e => erros.push(String(e)));
  // Pedidos da tela ao agente: o menu do `:` cobre a linha de estado, e isso nao pode virar
  // «nenhum arquivo» seguido de releitura a cada comando digitado.
  const pedidos = { arquivo: 0, simbolos: 0 };
  page.on('request', r => { const u = r.url(); if (u.includes('/v1/ide/arquivo')) pedidos.arquivo++; if (u.includes('/v1/ide/simbolos')) pedidos.simbolos++; });
  await page.goto(`${ORIG}/?tela=ide`);
  await page.waitForSelector('#tela-ide:not([hidden])');
  await page.click('#ideAbrirHelix');
  let espelho = '';
  for (let i = 0; i < 60 && !/NOR/.test(espelho); i++) { await esperar(250); espelho = await page.textContent('#termEspelho'); }
  check('o Helix sobe e desenha a linha de estado', /NOR/.test(espelho), espelho.slice(0, 120));

  const painel = () => page.$eval('#ideMinimapa', p => ({ ...p.dataset, hidden: p.hidden, estado: document.getElementById('minimapaEstado').textContent, aviso: document.getElementById('minimapaEstado').hasAttribute('data-aviso') }));
  const ate = async (f, n = 40) => { let v; for (let i = 0; i < n; i++) { v = await painel(); if (f(v)) return v; await esperar(250); } return v; };
  const helix = async cmd => { await page.click('#termCanvas'); await page.keyboard.press('Escape'); await esperar(120); await page.keyboard.type(cmd); await page.keyboard.press('Enter'); };

  await helix(':open src/longo.rs');
  const p1 = await ate(v => v.modo === 'faixa');
  // Pixels pintados no canvas: o mapa desenhou o texto, nao so a moldura.
  const tinta = await page.$eval('#minimapaCanvas', c => { const d = c.getContext('2d').getImageData(0, 0, c.width, c.height).data; let n = 0; for (let i = 3; i < d.length; i += 4) if (d[i] > 0) n++; return { n, w: c.width, h: c.height }; });
  check('minimapa: aparece com o Helix e desenha o arquivo (pixels pintados no canvas)', !p1.hidden && tinta.n > 500, JSON.stringify({ ...tinta, modo: p1.modo }));
  check('minimapa: faixa visivel lida da calha (comeca na linha 1, cursor na 1)', p1.modo === 'faixa' && p1.de === '1' && Number(p1.ate) > 10 && Number(p1.ate) < TOTAL && p1.cursor === '1', JSON.stringify(p1));
  await page.screenshot({ path: join(OUT, 'ide_minimapa_escuro.png') });
  capturas.push('ide_minimapa_escuro.png');

  // Clique a 3/4 da altura do mapa: o `:goto` sai, o Helix vai, e a faixa acompanha.
  const caixa = await page.$eval('#minimapaCanvas', c => { const r = c.getBoundingClientRect(); return { x: r.left, y: r.top, w: r.width, h: r.height }; });
  await page.mouse.click(caixa.x + caixa.w / 2, caixa.y + caixa.h * 0.75);
  const alvo = Number(await page.$eval('#minimapaCanvas', c => c.dataset.goto));
  const p2 = await ate(v => v.cursor === String(alvo) && v.modo === 'faixa');
  check('clique no mapa manda :goto N e o Helix vai (cursor da linha de estado = N)', alvo > 100 && p2.cursor === String(alvo), `alvo ${alvo} | ${JSON.stringify(p2)}`);
  check('a faixa acompanha o cursor (de <= N <= ate, faixa saiu do topo)', Number(p2.de) > 1 && Number(p2.de) <= alvo && alvo <= Number(p2.ate), JSON.stringify(p2));

  // A faixa DESENHADA, nao so o dataset: le os pixels da coluna da borda direita do canvas e
  // acha as linhas pintadas com a cor da faixa (--acao-consultar). O topo do contorno tem de
  // cair na linha `de` do dataset (+-2 linhas), senao o estado diz uma coisa e a tela outra.
  const faixaPx = await page.$eval('#minimapaCanvas', c => {
    const cor = getComputedStyle(document.documentElement).getPropertyValue('--acao-consultar').trim();
    const t = document.createElement('canvas').getContext('2d'); t.fillStyle = cor; t.fillRect(0, 0, 1, 1);
    const [R, G, B] = t.getImageData(0, 0, 1, 1).data;
    const d = c.getContext('2d').getImageData(0, 0, c.width, c.height).data;
    const x = c.width - 1, ys = [];
    for (let y = 0; y < c.height; y++) { const i = (y * c.width + x) * 4; if (d[i + 3] > 120 && Math.abs(d[i] - R) + Math.abs(d[i + 1] - G) + Math.abs(d[i + 2] - B) < 60) ys.push(y); }
    const dpr = window.devicePixelRatio || 1, H = c.getBoundingClientRect().height;
    const total = Number(c.getAttribute('aria-valuemax')); const p = Math.min(3, H / Math.max(1, total));
    return { ys: ys.length, topo: ys.length ? Math.floor(ys[0] / dpr / p) + 1 : null, base: ys.length ? Math.floor(ys.at(-1) / dpr / p) + 1 : null, cor };
  });
  check('a faixa DESENHADA cai onde o dataset diz (topo do contorno = de, +-2 linhas)', faixaPx.topo !== null && Math.abs(faixaPx.topo - Number(p2.de)) <= 2 && Math.abs(faixaPx.base - Number(p2.ate)) <= 2, `${JSON.stringify(faixaPx)} de ${p2.de} ate ${p2.ate}`);

  // Teclado: o mapa e um slider; End leva a ultima linha.
  await page.focus('#minimapaCanvas');
  await page.keyboard.press('End');
  const p3 = await ate(v => v.cursor === String(TOTAL));
  const aria = await page.$eval('#minimapaCanvas', c => [c.getAttribute('aria-valuenow'), c.getAttribute('aria-valuemax')]);
  check('teclado: End no mapa leva o Helix a ultima linha e o aria-valuenow acompanha', p3.cursor === String(TOTAL) && aria[0] === String(TOTAL) && aria[1] === String(TOTAL), `${JSON.stringify(p3)} aria ${aria}`);

  // Tema claro: o mesmo mapa, tintas dos tokens do claro.
  await page.click('#trocarTema');
  await esperar(400);
  const temaClaro = await page.evaluate(() => document.documentElement.dataset.tema);
  const tintaClaro = await page.$eval('#minimapaCanvas', c => { const d = c.getContext('2d').getImageData(0, 0, c.width, c.height).data; let n = 0; for (let i = 3; i < d.length; i += 4) if (d[i] > 0) n++; return n; });
  check('tema claro: o mapa se repinta (pixels pintados)', temaClaro === 'claro' && tintaClaro > 500, `${temaClaro} ${tintaClaro}`);
  await page.screenshot({ path: join(OUT, 'ide_minimapa_claro.png') });
  capturas.push('ide_minimapa_claro.png');
  await page.click('#trocarTema');
  await esperar(300);

  // O risco nomeado pelo pesquisador: numeracao relativa quebra a raspagem da calha. O painel
  // cai para «so cursor» (o cursor continua vindo da linha de estado) e diz por que.
  const antes = { ...pedidos };
  await helix(':set line-number relative');
  const p4 = await ate(v => v.modo === 'cursor');
  check('line-number = relative: o painel cai para «so cursor», sem faixa, com aviso', p4.modo === 'cursor' && p4.de === '' && p4.cursor === String(TOTAL) && p4.aviso, JSON.stringify(p4));
  await page.screenshot({ path: join(OUT, 'ide_minimapa_relativo.png') });
  capturas.push('ide_minimapa_relativo.png');
  await helix(':set line-number absolute');
  const p5 = await ate(v => v.modo === 'faixa');
  check('de volta a absoluta: a faixa volta', p5.modo === 'faixa' && Number(p5.ate) === TOTAL, JSON.stringify(p5));
  check('digitar dois comandos no `:` nao rele o arquivo nem os simbolos (0 pedidos)', pedidos.arquivo === antes.arquivo && pedidos.simbolos === antes.simbolos, JSON.stringify({ antes, depois: pedidos }));

  // Limite declarado: alteracao nao salva nao aparece no mapa; o painel avisa, e salvar rele.
  await helix(':goto 1');
  await page.keyboard.press('Escape');
  await page.keyboard.type('Onova linha');
  const p6 = await ate(v => v.aviso && v.modo === 'faixa');
  const total6 = await page.$eval('#minimapaCanvas', c => c.getAttribute('aria-valuemax'));
  check('nao salvo: o mapa segue o disco e o painel avisa', p6.aviso && total6 === String(TOTAL), `${JSON.stringify(p6)} max ${total6}`);
  await helix(':w');
  let total7 = '';
  for (let i = 0; i < 40 && total7 !== String(TOTAL + 1); i++) { await esperar(250); total7 = await page.$eval('#minimapaCanvas', c => c.getAttribute('aria-valuemax')); }
  check('salvou (:w): o mapa rele o disco sozinho (+1 linha)', total7 === String(TOTAL + 1), total7);

  check('o limite esta escrito na tela (nota do painel pela fabrica)', await page.$eval('#ideMinimapa .minimapa-nota', n => n.dataset.txt === 'ide.minimapa_limite' && n.textContent.length > 20));
  check('sem erro de JavaScript na pagina', erros.length === 0, erros.join(' | '));
  await helix(':q!');
  await browser.close();
} catch (e) {
  falhou = e;
} finally {
  agente.kill('SIGTERM');
  rmSync(D, { recursive: true, force: true });
}
if (falhou) { console.log('FALHA', falhou.stack || falhou.message); console.log(log.agente.slice(-1500)); checagens.push({ nome: 'excecao', ok: false, detalhe: String(falhou.message) }); }
const passou = checagens.filter(c => c.ok).length;
writeFileSync(join(OUT, 'ide_minimapa.json'), JSON.stringify({ roteiro: 'tests/desktop/ide_minimapa.mjs', medido_em: new Date().toISOString(), placar: `${passou}/${checagens.length}`, ok: passou === checagens.length && checagens.length > 0, checagens, capturas }, null, 1));
console.log(`placar: ${passou}/${checagens.length}`);
process.exit(checagens.length && passou === checagens.length ? 0 : 1);
