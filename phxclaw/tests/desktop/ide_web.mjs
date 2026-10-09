// O IDE no navegador com o agente REAL (`phxclaw servir`), sem Tauri: a tela IDE do PWA
// abre o Helix pelo websocket /v1/ide/terminal, o prompt do Helix aparece na grade (a linha
// de estado `NOR`), a trilha mostra o caminho e >= 1 simbolo de um .rs (rust-analyzer pelo
// /v1/ide/simbolos), e `:q` encerra a sessao. E tambem a prova de que o websocket recusa o
// token errado e que so o Helix abre por ele.
//
// Uso: PLAYWRIGHT_BROWSERS_PATH=/opt/pw-browsers node tests/desktop/ide_web.mjs [BINARIO] [--ui DIR]
// RED medido: copia da UI com o adaptador websocket desligado (app.js, `: comHttp ? (() => {`
// virando `: false ? (() => {`) derruba o Helix no canvas, a trilha e o :q.
// Exige hx (/opt/helix) e rust-analyzer no hospedeiro; sem eles sai 1 dizendo qual falta.
import { createRequire } from 'node:module';
import { spawn, execSync } from 'node:child_process';
import { mkdtempSync, writeFileSync, mkdirSync, rmSync, existsSync, readFileSync } from 'node:fs';
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
  checagens.push(!!ok);
  console.log(`${ok ? 'ok   ' : 'FALHA '}${nome}${detalhe ? ` :: ${detalhe}` : ''}`);
}
const esperar = ms => new Promise(r => setTimeout(r, ms));
const hx = existsSync('/opt/helix/hx') || process.env.PHXCLAW_HX;
if (!hx) { console.log('FALHA hx ausente (tools/instalar_helix.sh)'); process.exit(1); }

const D = mkdtempSync(join(tmpdir(), 'phx-ide-web-'));
const PROJ = join(D, 'projeto');
mkdirSync(join(PROJ, 'src'), { recursive: true });
writeFileSync(join(PROJ, 'Cargo.toml'), '[package]\nname = "calc"\nversion = "0.1.0"\nedition = "2021"\n\n[dependencies]\n');
writeFileSync(join(PROJ, 'src/main.rs'), 'struct Config {\n    porta: u16,\n}\n\nfn soma(a: i32, b: i32) -> i32 {\n    a + b\n}\n\nfn main() {\n    let c = Config { porta: 1 };\n    println!("{} {}", soma(1, 2), c.porta);\n}\n\n#[cfg(test)]\nmod t {\n    #[test]\n    fn soma_ok() {\n        assert_eq!(super::soma(1, 2), 3);\n    }\n}\n');
try { execSync('git init -q && git add -A && git -c user.email=t@t -c user.name=t commit -qm inicio', { cwd: PROJ, stdio: 'ignore' }); } catch {}
const token = 'token-ide-web-' + Date.now();
const PASTA = join(D, 'agente');
mkdirSync(PASTA, { recursive: true });
writeFileSync(join(PASTA, 'api.token'), token);
const porta = 18700 + Math.floor(Math.random() * 1000);
const log = { agente: '' };
const agente = spawn(BIN, ['servir', '--porta', String(porta), '--pasta', PASTA], {
  // A loja: o registro de exemplo do repositorio (0 lancamentos) e o trust store oficial --
  // a lista vem vazia do motor REAL, e a tela diz «catalogo vazio», nao 404.
  env: { PATH: process.env.PATH, HOME: D, PHXCLAW_PROJETO: PROJ, ...(UI_DIR ? { PHXCLAW_UI_DIR: UI_DIR } : {}), PHXCLAW_PACOTES_CATALOGO: join(RAIZ, 'config/community/registry.json'), PHXCLAW_PACOTES_DIR: join(D, 'pacotes'), PHXCLAW_PLUGIN_SIGNERS: join(RAIZ, 'config/trust/plugin-signers.json'), PHXCLAW_MODELO: 'ollama:falso', OLLAMA_HOST: 'http://127.0.0.1:1', PHXCLAW_API_TOKEN: token, CARGO_HOME: process.env.CARGO_HOME || '/root/.cargo', RUSTUP_HOME: process.env.RUSTUP_HOME || '/root/.rustup' },
  stdio: ['ignore', 'pipe', 'pipe'],
});
agente.stdout.on('data', b => { log.agente += b; });
agente.stderr.on('data', b => { log.agente += b; });
let falhou = null;
try {
  let vivo = false;
  for (let i = 0; i < 100 && !vivo; i++) {
    await esperar(200);
    try { vivo = (await fetch(`http://127.0.0.1:${porta}/health`)).ok; } catch {}
  }
  check('o agente real sobe e serve a UI', vivo, log.agente.split('\n').slice(-3).join(' | '));
  const ORIG = `http://127.0.0.1:${porta}`;

  // Websocket cru: token errado e recusado antes de qualquer terminal; bash e recusado.
  const ws = (msgs, espera = 3000) => new Promise(ok => {
    const s = new WebSocket(`ws://127.0.0.1:${porta}/v1/ide/terminal`);
    const vistos = [];
    s.onopen = () => { for (const m of msgs) s.send(JSON.stringify(m)); };
    s.onmessage = e => vistos.push(JSON.parse(e.data));
    s.onclose = () => ok(vistos);
    setTimeout(() => { try { s.close(); } catch {} }, espera);
  });
  const ruim = await ws([{ op: 'auth', token: 'errado' }, { op: 'abrir', programa: 'helix', colunas: 80, linhas: 24 }]);
  check('websocket: token errado e recusado (erro e fim), sem terminal', ruim.length === 1 && /token/.test(ruim[0].erro) && !ruim.some(m => m.ev === 'aberto'), JSON.stringify(ruim));
  const bash = await ws([{ op: 'auth', token }, { op: 'abrir', programa: 'bash', colunas: 80, linhas: 24 }], 1500);
  check('websocket: so o Helix abre pelo navegador (bash recusado com motivo)', bash.some(m => m.ev === 'pronto') && bash.some(m => m.ev === 'erro' && /Helix/.test(m.erro)) && !bash.some(m => m.ev === 'aberto'), JSON.stringify(bash).slice(0, 200));

  const browser = await chromium.launch();
  const ctx = await browser.newContext({ viewport: { width: 1366, height: 768 }, locale: 'pt-BR' });
  await ctx.addInitScript(t => { try { localStorage.setItem('phxclaw.token', t); } catch {} }, token);
  // A CSP do agente (connect-src 'self') tem de deixar passar o websocket do terminal.
  const vigia = await vigiarCsp(ctx);
  const page = await ctx.newPage();
  const erros = [];
  page.on('pageerror', e => erros.push(String(e)));
  await page.goto(`${ORIG}/?tela=ide`);
  await page.waitForSelector('#tela-ide:not([hidden])');
  const semTauri = await page.evaluate(() => !window.__TAURI__);
  check('a tela IDE abre no navegador, sem Tauri', semTauri);
  await page.click('#ideAbrirHelix');
  // O prompt do Helix: a linha de estado `NOR` desenhada na grade (lida pelo espelho).
  let espelho = '';
  for (let i = 0; i < 60 && !/NOR/.test(espelho); i++) { await esperar(250); espelho = await page.textContent('#termEspelho'); }
  const espelhoHtml = await page.$eval('#termEspelho', e => e.outerHTML.slice(0, 160));
  check('o Helix sobe no PTY do agente e o prompt (NOR) chega ao canvas pelo websocket', /NOR/.test(espelho), `${espelho.slice(0, 120)} | ${espelhoHtml}`);
  const status = await page.textContent('#ideStatus');
  check('o status mostra pid e a pasta do projeto', /pid \d+/.test(status) && status.includes(PROJ), status);
  await page.screenshot({ path: join(OUT, 'ide_web_helix.png') });

  // Abre um .rs pela linha de comando do Helix; a trilha le o arquivo da linha de estado.
  await page.click('#termCanvas');
  await page.keyboard.press('Escape');
  await page.keyboard.type(':open src/main.rs');
  await page.keyboard.press('Enter');
  let caminho = [];
  for (let i = 0; i < 40 && caminho.join('/') !== 'src/main.rs'; i++) { await esperar(250); caminho = await page.$$eval('#ideTrilha .trilha-caminho li', ls => ls.map(l => l.textContent)); }
  check('trilha: o caminho do arquivo aberto no Helix (src/main.rs)', caminho.join('/') === 'src/main.rs', caminho.join('/'));
  let simbolos = [];
  // O rust-analyzer precisa indexar o projeto (sysroot incluido): ate 120 s.
  for (let i = 0; i < 480 && !simbolos.length; i++) {
    await esperar(250);
    simbolos = await page.$$eval('#ideTrilha .trilha-sim', bs => bs.map(b => `${b.dataset.tipo}:${b.textContent}`));
    const nota = await page.$eval('#ideTrilha', t => t.querySelector('.trilha-nota')?.textContent || '').catch(() => '');
    if (/indisponíveis|unavailable/.test(nota)) { simbolos = [`ERRO ${nota}`]; break; }
  }
  check('trilha: >= 1 simbolo do .rs pelo rust-analyzer (/v1/ide/simbolos)', simbolos.length >= 1 && !simbolos[0].startsWith('ERRO') && simbolos.some(s => /soma|main|Config/.test(s)), simbolos.join(' ').slice(0, 200));
  await page.screenshot({ path: join(OUT, 'ide_web_trilha.png') });

  // Explorador de testes pela API real: a arvore do cargo (no sandbox) e o rodar de um no.
  await page.evaluate(() => { document.getElementById('ideTestes').open = true; });
  await page.click('#testesListar');
  let nos = [];
  for (let i = 0; i < 480 && !nos.some(n => /soma_ok/.test(n)); i++) {
    await esperar(250);
    nos = await page.$$eval('#testesArvore .no', ns => ns.map(n => `${n.dataset.tipo}:${n.dataset.no}`));
    const st = await page.textContent('#testesStatus');
    if (/erro|error|indispon/i.test(st) && !/Lendo|Reading/.test(st)) { nos = [`ERRO ${st}`]; break; }
  }
  check('explorador de testes: GET /v1/ide/testes lista calc -> t -> soma_ok pelo motor real', nos.includes('crate:calc') && nos.includes('teste:calc/t::soma_ok'), nos.join(' ').slice(0, 200));
  await page.click('#testesArvore button[data-rodar="calc/t::soma_ok"]', { timeout: 3000 }).catch(() => {});
  let marca = null;
  for (let i = 0; i < 480 && !marca; i++) { await esperar(250); marca = await page.$eval('#testesArvore', a => a.querySelector('.ok,.erro')?.textContent || null); }
  const stRodar = await page.textContent('#testesStatus');
  check('explorador de testes: POST /v1/ide/testes/rodar roda o no e marca PASSOU', marca === 'PASSOU' && /1 ok, 0 falhou/.test(stRodar), `${marca} | ${stRodar}`);
  await page.screenshot({ path: join(OUT, 'ide_web_testes.png') });

  // Loja de plugins pela API real (registro de exemplo, 0 lancamentos): 200 e «catalogo vazio».
  const st200 = await page.evaluate(async t => { const r = await fetch('./v1/plugins/catalogo', { headers: { Authorization: `Bearer ${t}` } }); return [r.status, await r.text()]; }, token);
  check('loja: GET /v1/plugins/catalogo responde 200 com a lista do motor real', st200[0] === 200 && /"plugins":\s*\[\]/.test(st200[1]), JSON.stringify(st200).slice(0, 160));
  await page.evaluate(() => mostrarTela('ferramentas'));
  await page.waitForTimeout(500);
  await page.evaluate(() => { document.getElementById('ferramentasPlugins').open = true; });
  await page.click('#pluginsCarregar');
  let stLoja = '';
  for (let i = 0; i < 40 && !/vazio|empty/i.test(stLoja); i++) { await esperar(250); stLoja = await page.textContent('#pluginsStatus'); }
  check('loja: a tela mostra «catalogo vazio» (nao 404) com o agente real', /vazio|empty/i.test(stLoja), stLoja);
  await page.screenshot({ path: join(OUT, 'ide_web_loja.png') });
  await page.evaluate(() => mostrarTela('ide'));
  await page.waitForTimeout(400);

  // `:q` encerra o Helix; a aba fica morta e o status diz encerrado.
  await page.click('#termCanvas');
  await page.keyboard.press('Escape');
  await page.keyboard.type(':q!');
  await page.keyboard.press('Enter');
  let fim = '';
  for (let i = 0; i < 40 && !/encerrado|ended/.test(fim); i++) { await esperar(250); fim = await page.textContent('#ideStatus'); }
  check('digitar :q encerra a sessao do Helix (status diz encerrado)', /encerrado|ended/.test(fim), fim);
  check('sem erro de JavaScript na pagina', erros.length === 0, erros.join(' | ').slice(0, 300));
  const csp = await vigia.todas(ctx.pages());
  check('zero violacao de CSP (websocket do terminal sob connect-src \'self\')', csp.length === 0, csp.slice(0, 3).join(' | '));
  await browser.close();
} catch (e) {
  falhou = e;
} finally {
  agente.kill('SIGTERM');
  rmSync(D, { recursive: true, force: true });
}
if (falhou) { console.log('FALHA', falhou.message); console.log(log.agente.slice(-2000)); checagens.push(false); }
console.log(`placar: ${checagens.filter(Boolean).length}/${checagens.length}`);
process.exit(checagens.length && checagens.every(Boolean) ? 0 : 1);
