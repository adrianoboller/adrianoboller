// O terminal compartilhado do IDE web (gap `live_share`, decisao do dono: Live Share =
// terminal compartilhado) com o agente REAL e o Helix REAL, em DOIS navegadores separados:
// o anfitriao compartilha pela tela e digita; o convidado abre o link do convite, ve a grade
// do anfitriao e NAO consegue digitar (o websocket dele nao manda nada alem do `entrar`); o
// Helix comum nao se compartilha (R1: tem o token da API no ambiente) e a tela oferece
// reabrir sem a credencial; revogar derruba o convidado na hora e o mesmo convite e recusado
// depois; o convite expirado derruba quem esta dentro e recusa quem chega.
//
// Uso: PLAYWRIGHT_BROWSERS_PATH=/opt/pw-browsers node tests/desktop/ide_compartilhar.mjs [BINARIO] [--ui DIR]
// Resultado em tests/desktop/out/ide_compartilhar.json; capturas ide_compartilhar_{anfitriao,convidado,revogado}.png.
// Exige hx (/opt/helix); sem ele sai 1 dizendo que falta.
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

const D = mkdtempSync(join(tmpdir(), 'phx-ide-comp-'));
const PROJ = join(D, 'projeto');
mkdirSync(join(PROJ, 'src'), { recursive: true });
writeFileSync(join(PROJ, 'src/main.rs'), 'fn main() {\n    println!("oi");\n}\n');
const token = 'token-ide-compartilhar-' + Date.now();
const PASTA = join(D, 'agente');
mkdirSync(PASTA, { recursive: true });
writeFileSync(join(PASTA, 'api.token'), token);
const porta = 19700 + Math.floor(Math.random() * 1000);
const log = { agente: '' };
const agente = spawn(BIN, ['servir', '--porta', String(porta), '--pasta', PASTA], {
  env: { PATH: process.env.PATH, HOME: D, PHXCLAW_PROJETO: PROJ, PHXCLAW_LSP: '0', ...(UI_DIR ? { PHXCLAW_UI_DIR: UI_DIR } : {}), PHXCLAW_MODELO: 'ollama:falso', OLLAMA_HOST: 'http://127.0.0.1:1', PHXCLAW_API_TOKEN: token },
  stdio: ['ignore', 'pipe', 'pipe'],
});
agente.stdout.on('data', b => { log.agente += b; });
agente.stderr.on('data', b => { log.agente += b; });
const capturas = [];
let falhou = null;
const navegadores = [];
try {
  let vivo = false;
  for (let i = 0; i < 100 && !vivo; i++) {
    await esperar(200);
    try { vivo = (await fetch(`http://127.0.0.1:${porta}/health`)).ok; } catch {}
  }
  check('o agente real sobe e serve a UI', vivo, log.agente.split('\n').slice(-3).join(' | '));
  const ORIG = `http://127.0.0.1:${porta}`;
  // Pagina com captura de erro de JavaScript e de violacao de CSP.
  const abrirPagina = async (b, opcoes, comToken) => {
    const ctx = await b.newContext({ locale: 'pt-BR', ...opcoes });
    await ctx.addInitScript(t => { try { if (t) localStorage.setItem('phxclaw.token', t); localStorage.removeItem('phxclaw.tema'); } catch {} }, comToken ? token : '');
    const page = await ctx.newPage();
    page.erros = [];
    page.on('pageerror', e => page.erros.push(String(e)));
    page.on('console', m => { if (m.type() === 'error' && /Content Security Policy/.test(m.text())) page.erros.push(m.text()); });
    return page;
  };
  const espelhoAte = async (page, re, n = 40) => { let t = ''; for (let i = 0; i < n && !re.test(t); i++) { await esperar(250); t = await page.textContent('#termEspelho'); } return t; };
  const textoAte = async (page, sel, re, n = 40) => { let t = ''; for (let i = 0; i < n && !re.test(t); i++) { await esperar(250); t = (await page.textContent(sel)) || ''; } return t; };

  // ---------------------------------------------------------------- anfitriao
  const navA = await chromium.launch();
  navegadores.push(navA);
  const anf = await abrirPagina(navA, { viewport: { width: 1280, height: 860 }, colorScheme: 'dark' }, true);
  await anf.goto(`${ORIG}/?tela=ide`);
  await anf.waitForSelector('#tela-ide:not([hidden])');
  await anf.click('#ideAbrirHelix');
  check('anfitriao: o Helix sobe', /NOR/.test(await espelhoAte(anf, /NOR/)));
  await anf.click('#ideCompartilhar > summary');
  check('anfitriao: sem compartilhar, o painel diz «Não compartilhado» e o selo some', /Não compartilhado/.test(await anf.textContent('#compEstado')) && await anf.$eval('#ideCompartilhado', e => e.hidden));
  // R1: o Helix comum tem a credencial da completacao; compartilhar recusa e oferece reabrir.
  await anf.click('#compCriar');
  const recusa = await textoAte(anf, '#compEstado', /Reabra-o sem a credencial/);
  const reabrirVisivel = await anf.$eval('#compReabrir', b => !b.hidden);
  check('anfitriao: o Helix COM a credencial da IA nao se compartilha; a tela explica e oferece reabrir', /Reabra-o sem a credencial/.test(recusa) && reabrirVisivel, recusa);
  await anf.click('#compReabrir');
  let link = '';
  for (let i = 0; i < 60 && !link.includes('#convite='); i++) { await esperar(250); link = await anf.$eval('#compLink', e => e.value); }
  check('anfitriao: REABRIR abre o Helix compartilhavel e o convite sai (link com #convite=SESSAO.TOKEN de 32 bytes)', /#convite=[0-9a-f-]{36}\.[0-9a-f]{64}$/.test(link), link.replace(/[0-9a-f]{64}$/, '<token>'));
  check('anfitriao: o Helix reaberto sobe', /NOR/.test(await espelhoAte(anf, /NOR/)));

  // ---------------------------------------------------------------- convidado (outro navegador, sem token)
  const navB = await chromium.launch();
  navegadores.push(navB);
  const conv = await abrirPagina(navB, { viewport: { width: 400, height: 860 }, colorScheme: 'light' }, false);
  const quadros = [];
  conv.on('websocket', ws => { if (ws.url().includes('/v1/ide/compartilhado')) ws.on('framesent', f => quadros.push(String(f.payload))); });
  await conv.goto(link);
  await conv.waitForSelector('#tela-ide:not([hidden])');
  const urlDepois = conv.url();
  check('convidado: o link abre a tela IDE e o convite SAI da URL (fica so na memoria da aba)', !urlDepois.includes('convite') && urlDepois.endsWith('#ide'), urlDepois);
  const seloConv = await textoAte(conv, '#ideCompartilhado', /somente leitura/);
  check('convidado: o selo diz que assiste a um terminal compartilhado, somente leitura', /assiste a um terminal compartilhado — somente leitura/.test(seloConv), seloConv);
  check('convidado: recebe a grade do anfitriao (linha de estado do Helix no espelho)', /NOR/.test(await espelhoAte(conv, /NOR/)));
  const seloAnf = await textoAte(anf, '#ideCompartilhado', /Compartilhado com 1/);
  const itens = await anf.$$eval('#compConvidados li', ls => ls.length);
  check('anfitriao: o selo permanente conta o convidado e a lista o mostra com REVOGAR', /Compartilhado com 1 de 4/.test(seloAnf) && itens === 1, `${seloAnf} | ${itens}`);

  // O anfitriao digita; o convidado ve.
  await anf.click('#termCanvas');
  await anf.keyboard.press('Escape');
  await esperar(150);
  await anf.keyboard.type('iMARCA-DO-ANFITRIAO');
  const viu = await espelhoAte(conv, /MARCA-DO-ANFITRIAO/);
  check('convidado: ve o que o anfitriao digita (a linha do cursor no espelho do convidado)', /MARCA-DO-ANFITRIAO/.test(viu), viu.slice(0, 160));
  await anf.keyboard.press('Escape');
  await anf.screenshot({ path: join(OUT, 'ide_compartilhar_anfitriao.png') });
  capturas.push('ide_compartilhar_anfitriao.png');

  // O convidado tenta digitar e colar: nada sai pelo fio dele, nada chega ao terminal.
  await conv.click('#termCanvas');
  await conv.keyboard.type('iXYZ-DO-CONVIDADO');
  await conv.keyboard.press('Enter');
  await esperar(1200);
  const anfVe = await anf.textContent('#termEspelho');
  const convVe = await conv.textContent('#termEspelho');
  check('convidado: o teclado do convidado nao chega ao terminal (nem no anfitriao nem na propria grade)', !/XYZ-DO-CONVIDADO/.test(anfVe) && !/XYZ-DO-CONVIDADO/.test(convVe), `${anfVe.slice(0, 120)} | ${convVe.slice(0, 120)}`);
  check('convidado: o websocket do convidado so mandou o «entrar» (1 quadro, sem tecla)', quadros.length === 1 && JSON.parse(quadros[0]).op === 'entrar', JSON.stringify(quadros.map(q => JSON.parse(q).op)));
  check('convidado: o botao de abrir terminal fica desligado', await conv.$eval('#ideAbrirHelix', b => b.disabled));
  await conv.screenshot({ path: join(OUT, 'ide_compartilhar_convidado.png') });
  capturas.push('ide_compartilhar_convidado.png');

  // Revogar pela tela: o convidado cai na hora, e o mesmo convite e recusado depois.
  await anf.click('#compRevogar');
  const fimConv = await textoAte(conv, '#ideCompartilhado', /terminou: revogado/);
  check('revogar: o convidado recebe o fim («revogado») sem recarregar', /terminou: revogado/.test(fimConv), fimConv);
  const fimAnf = await textoAte(anf, '#compEstado', /encerrado: revogado/);
  check('revogar: o anfitriao ve o compartilhamento encerrado e o selo some', /encerrado: revogado/.test(fimAnf) && await anf.$eval('#ideCompartilhado', e => e.hidden), fimAnf);
  await conv.screenshot({ path: join(OUT, 'ide_compartilhar_revogado.png') });
  capturas.push('ide_compartilhar_revogado.png');
  const tarde = await abrirPagina(navB, { viewport: { width: 400, height: 860 } }, false);
  await tarde.goto(link);
  const recusaTarde = await textoAte(tarde, '#ideCompartilhado', /recusado/);
  check('revogado: o mesmo link, aberto depois, e recusado («Convite recusado»)', /Convite recusado: convite inexistente, revogado ou encerrado/.test(recusaTarde), recusaTarde);

  // Expiracao dura (pela rota, com 3 s): quem esta dentro cai, quem chega depois e recusado.
  const r = await fetch(`${ORIG}/v1/ide/compartilhar`, { method: 'POST', headers: { Authorization: `Bearer ${token}`, 'Content-Type': 'application/json' }, body: JSON.stringify({ expira_em_s: 3 }) });
  const v = await r.json();
  const linkCurto = `${ORIG}/#convite=${v.convite}`;
  const curto = await abrirPagina(navB, { viewport: { width: 1280, height: 860 } }, false);
  await curto.goto(linkCurto);
  const entrou = await textoAte(curto, '#ideCompartilhado', /somente leitura/, 20);
  const expirou = await textoAte(curto, '#ideCompartilhado', /terminou: expirado/, 40);
  check('expiracao: o convidado de um convite de 3 s entra e cai com «expirado»', /somente leitura/.test(entrou) && /terminou: expirado/.test(expirou), `${entrou} | ${expirou}`);
  const vencido = await abrirPagina(navB, { viewport: { width: 1280, height: 860 } }, false);
  await vencido.goto(linkCurto);
  const recusaVencido = await textoAte(vencido, '#ideCompartilhado', /recusado/);
  check('expiracao: o convite vencido e recusado a quem chega depois', /Convite recusado/.test(recusaVencido), recusaVencido);

  for (const [nome, p] of [['anfitriao', anf], ['convidado', conv], ['tarde', tarde], ['curto', curto], ['vencido', vencido]]) {
    check(`${nome}: sem erro de JavaScript nem violacao de CSP`, p.erros.length === 0, p.erros.join(' | '));
  }
  await anf.click('#termCanvas');
  await anf.keyboard.press('Escape');
  await anf.keyboard.type(':qa!');
  await anf.keyboard.press('Enter');
} catch (e) {
  falhou = e;
} finally {
  for (const b of navegadores) await b.close().catch(() => {});
  agente.kill('SIGTERM');
  rmSync(D, { recursive: true, force: true });
}
if (falhou) { console.log('FALHA', falhou.stack || falhou.message); console.log(log.agente.slice(-1500)); checagens.push({ nome: 'excecao', ok: false, detalhe: String(falhou.message) }); }
const passou = checagens.filter(c => c.ok).length;
writeFileSync(join(OUT, 'ide_compartilhar.json'), JSON.stringify({ roteiro: 'tests/desktop/ide_compartilhar.mjs', medido_em: new Date().toISOString(), placar: `${passou}/${checagens.length}`, ok: passou === checagens.length && checagens.length > 0, checagens, capturas }, null, 1));
console.log(`placar: ${passou}/${checagens.length}`);
process.exit(checagens.length && passou === checagens.length ? 0 : 1);
