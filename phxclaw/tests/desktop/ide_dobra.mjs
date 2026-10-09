// A dobra de codigo do IDE web (gap `dobra_codigo` do VS Code) com o agente REAL
// (`phxclaw servir`), o Helix REAL no PTY e o rust-analyzer REAL no bwrap. O editor e o
// Helix 25.07, que nao dobra; quem dobra e o painel «Leitura com dobra», que le do disco o
// arquivo aberto no Helix e as regioes do servidor de linguagem (/v1/ide/dobras,
// textDocument/foldingRange; sem servidor, a reserva por chaves ou indentacao, dita).
//
// Prova, nas larguras 1280 e 400 e nos dois temas: o painel fechado nao pede nada; aberto,
// a fonte e o LSP; dobrar a funcao `longa` esconde exatamente as linhas do corpo (contadas no
// DOM), a alca diz aria-expanded=false, desdobrar devolve; teclado (setas, ← → , Enter e
// Ctrl+Shift+[ ]); o estado e por arquivo (outro arquivo nao herda, voltar recupera); o texto
// do arquivo entra como TEXTO (um `<b>` no fonte nao vira elemento); o `.yaml` cai na reserva
// por indentacao e diz isso.
//
// Uso: PLAYWRIGHT_BROWSERS_PATH=/opt/pw-browsers node tests/desktop/ide_dobra.mjs [BINARIO] [--ui DIR]
// Resultado em tests/desktop/out/ide_dobra.json; capturas ide_dobra_{1280,400}_{escuro,claro}.png.
// Exige hx (/opt/helix) e rust-analyzer; sem eles sai 1 dizendo o que falta.
import { createRequire } from 'node:module';
import { spawn, execSync } from 'node:child_process';
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
try { execSync('rust-analyzer --version', { stdio: 'ignore' }); } catch { console.log('FALHA rust-analyzer ausente'); process.exit(1); }

const D = mkdtempSync(join(tmpdir(), 'phx-ide-dobra-'));
const PROJ = join(D, 'projeto');
mkdirSync(join(PROJ, 'src'), { recursive: true });
writeFileSync(join(PROJ, 'Cargo.toml'), '[package]\nname = "calc"\nversion = "0.1.0"\nedition = "2021"\n');
// `longa` comeca na linha 3 e o corpo vai da 4 a 13 (a `}` da 14 fica visivel); o `if` de
// dentro e uma regiao propria. A linha 2 tem HTML num comentario: tem de sair como texto.
const MAIN = [
  '// calculadora',
  '// <b>negrito</b> <img src=x onerror="window.__xss=1">',
  'fn longa(n: i32) -> i32 {',
  '    let mut t = 0;',
  '    for i in 0..n {',
  '        t += i;',
  '    }',
  '    if t > 10 {',
  '        t -= 1;',
  '        t *= 2;',
  '    }',
  '    let u = t + 1;',
  '    u',
  '}',
  '',
  'fn main() {',
  '    println!("{}", longa(5));',
  '}',
].join('\n') + '\n';
writeFileSync(join(PROJ, 'src/main.rs'), MAIN);
writeFileSync(join(PROJ, 'conf.yaml'), 'a:\n  b: 1\n  c:\n    d: 2\ne: 3\n');
const token = 'token-ide-dobra-' + Date.now();
const PASTA = join(D, 'agente');
mkdirSync(PASTA, { recursive: true });
writeFileSync(join(PASTA, 'api.token'), token);
const porta = 19700 + Math.floor(Math.random() * 1000);
const log = { agente: '' };
const agente = spawn(BIN, ['servir', '--porta', String(porta), '--pasta', PASTA], {
  env: { PATH: process.env.PATH, HOME: D, PHXCLAW_PROJETO: PROJ, ...(UI_DIR ? { PHXCLAW_UI_DIR: UI_DIR } : {}), PHXCLAW_MODELO: 'ollama:falso', OLLAMA_HOST: 'http://127.0.0.1:1', PHXCLAW_API_TOKEN: token, CARGO_HOME: process.env.CARGO_HOME || '/root/.cargo', RUSTUP_HOME: process.env.RUSTUP_HOME || '/root/.rustup' },
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
  const browser = await chromium.launch();

  for (const largura of [1280, 400]) {
    const ctx = await browser.newContext({ viewport: { width: largura, height: 860 }, locale: 'pt-BR', colorScheme: 'dark' });
    await ctx.addInitScript(t => { try { localStorage.setItem('phxclaw.token', t); localStorage.removeItem('phxclaw.tema'); } catch {} }, token);
    const page = await ctx.newPage();
    const erros = [];
    page.on('pageerror', e => erros.push(String(e)));
    page.on('console', m => { if (m.type() === 'error' && /Content Security Policy/.test(m.text())) erros.push(m.text()); });
    let pedidosDobras = 0;
    page.on('request', r => { if (r.url().includes('/v1/ide/dobras')) pedidosDobras++; });
    await page.goto(`${ORIG}/?tela=ide`);
    await page.waitForSelector('#tela-ide:not([hidden])');
    await page.click('#ideAbrirHelix');
    let espelho = '';
    for (let i = 0; i < 60 && !/NOR/.test(espelho); i++) { await esperar(250); espelho = await page.textContent('#termEspelho'); }
    check(`${largura}: o Helix sobe`, /NOR/.test(espelho), espelho.slice(0, 120));
    const helix = async cmd => { await page.click('#termCanvas'); await esperar(80); await page.keyboard.press('Escape'); await esperar(150); await page.keyboard.type(cmd); await page.keyboard.press('Enter'); };
    const painel = () => page.$eval('#ideLeitura', p => ({ ...p.dataset, hidden: p.hidden, open: p.open, estado: document.getElementById('leituraEstado').textContent }));
    const ate = async (f, n = 40) => { let v; for (let i = 0; i < n; i++) { v = await painel(); if (f(v)) return v; await esperar(250); } return v; };

    await helix(':open src/main.rs');
    const p0 = await ate(v => v.arquivo === 'src/main.rs');
    check(`${largura}: o painel aparece com o Helix e acompanha o arquivo aberto, FECHADO e sem pedir nada (0 pedidos)`, !p0.hidden && !p0.open && p0.arquivo === 'src/main.rs' && pedidosDobras === 0, `${JSON.stringify(p0)} pedidos ${pedidosDobras}`);
    await page.click('#ideLeitura > summary');
    // A primeira largura espera o rust-analyzer indexar; a segunda ja o encontra de pe.
    const p1 = await ate(v => v.modo === 'pronto' && v.origem === 'lsp', 200);
    check(`${largura}: aberto, as regioes vem do servidor de linguagem (origem lsp)`, p1.modo === 'pronto' && p1.origem === 'lsp' && Number(p1.regioes) >= 2, JSON.stringify(p1));
    const linhasVis = () => page.$$eval('#leituraCodigo .ll', ls => ls.filter(l => !l.hidden).map(l => Number(l.dataset.n)));
    const alca = n => `#leituraCodigo button.ll-dobra[data-inicio="${n}"]`;
    const fimLonga = Number(await page.$eval(alca(3), b => b.dataset.fim));
    // O rust-analyzer diz onde a regiao acaba (13 deixa a `}` visivel, 14 a esconde junto).
    check(`${largura}: a funcao longa tem alca na linha 3 e a regiao do servidor vai ate a } (13 ou 14)`, fimLonga === 13 || fimLonga === 14, `fim ${fimLonga}`);
    const corpo = Array.from({ length: fimLonga - 3 }, (_, i) => i + 4);
    const N = corpo.length;
    const xss = await page.evaluate(() => ({ b: document.querySelectorAll('#leituraCodigo b, #leituraCodigo img').length, x: window.__xss || 0, t: document.querySelector('#leituraCodigo .ll[data-n="2"] .ll-t')?.textContent }));
    check(`${largura}: o fonte entra como TEXTO (o <b> e o <img> do comentario nao viram elemento)`, xss.b === 0 && xss.x === 0 && xss.t.includes('<b>negrito</b>'), JSON.stringify(xss));

    // Clique: dobra a funcao. As linhas 4..13 somem do DOM visivel, a 3 e a 14 ficam.
    const antes = await linhasVis();
    await page.click(alca(3));
    const dob = await linhasVis();
    const sumiram = antes.filter(n => !dob.includes(n));
    const aria = await page.$eval(alca(3), b => [b.getAttribute('aria-expanded'), b.getAttribute('aria-label'), b.parentElement.querySelector('.ll-oculto').textContent]);
    check(`${largura}: clique dobra a funcao: somem exatamente as linhas 4–${fimLonga}`, JSON.stringify(sumiram) === JSON.stringify(corpo) && dob.includes(3) && dob.includes(fimLonga + 1), `sumiram ${sumiram}`);
    check(`${largura}: a alca diz aria-expanded=false, o rotulo pela fabrica e o selo «⋯ ${N} linhas»`, aria[0] === 'false' && aria[1].includes(`Desdobrar as linhas 4–${fimLonga}`) && aria[2].includes(`${N} linhas`), JSON.stringify(aria));
    const pDob = await painel();
    check(`${largura}: o estado do painel conta as linhas dobradas`, pDob.ocultas === String(N) && pDob.estado.includes(`${N} linhas dobradas`), JSON.stringify(pDob));
    await page.screenshot({ path: join(OUT, `ide_dobra_${largura}_escuro.png`) });
    capturas.push(`ide_dobra_${largura}_escuro.png`);
    // Tema claro: a mesma dobra; o codigo continua legivel (contraste medido da linha 3).
    await page.click('#trocarTema');
    await esperar(300);
    const contraste = await page.evaluate(() => {
      const t = document.querySelector('#leituraCodigo .ll[data-n="3"] .ll-t');
      const cor = s => s.match(/\d+(\.\d+)?/g).slice(0, 3).map(Number);
      const lum = c => { const f = v => { v /= 255; return v <= 0.03928 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4; }; const [r, g, b] = c.map(f); return 0.2126 * r + 0.7152 * g + 0.0722 * b; };
      const a = lum(cor(getComputedStyle(t).color)), b = lum(cor(getComputedStyle(document.getElementById('leituraCodigo')).backgroundColor));
      return { tema: document.documentElement.dataset.tema, razao: (Math.max(a, b) + 0.05) / (Math.min(a, b) + 0.05) };
    });
    check(`${largura}: tema claro mantem a dobra e o texto legivel (>= 4,5:1)`, contraste.tema === 'claro' && contraste.razao >= 4.5 && (await linhasVis()).length === dob.length, JSON.stringify(contraste));
    await page.screenshot({ path: join(OUT, `ide_dobra_${largura}_claro.png`) });
    capturas.push(`ide_dobra_${largura}_claro.png`);
    await page.click('#trocarTema');
    await esperar(200);

    // Clique de novo: desdobra, tudo volta.
    await page.click(alca(3));
    check(`${largura}: segundo clique desdobra (todas as linhas visiveis de novo)`, (await linhasVis()).length === antes.length, `${(await linhasVis()).length} de ${antes.length}`);

    // Teclado: ← dobra, → desdobra, Enter alterna, Ctrl+Shift+[ dobra, ↓ anda para a proxima alca.
    await page.focus(alca(3));
    await page.keyboard.press('ArrowLeft');
    const k1 = (await linhasVis()).length;
    await page.keyboard.press('ArrowRight');
    const k2 = (await linhasVis()).length;
    await page.keyboard.press('Enter');
    const k3 = (await linhasVis()).length;
    await page.keyboard.press('Enter');
    await page.keyboard.press('Control+Shift+BracketLeft');
    const k4 = (await linhasVis()).length;
    await page.keyboard.press('Control+Shift+BracketRight');
    await page.keyboard.press('ArrowDown');
    const focoDepois = await page.evaluate(() => [document.activeElement?.dataset?.inicio, document.activeElement?.tabIndex]);
    check(`${largura}: teclado: ← dobra, → desdobra, Enter alterna, Ctrl+Shift+[ dobra`, k1 === antes.length - N && k2 === antes.length && k3 === antes.length - N && k4 === antes.length - N, JSON.stringify({ k1, k2, k3, k4, total: antes.length }));
    check(`${largura}: teclado: ↓ leva o foco a proxima regiao (roving tabindex: so ela com tabIndex 0)`, Number(focoDepois[0]) > 3 && focoDepois[1] === 0, JSON.stringify(focoDepois));
    // A regiao de dentro (o `for` ou o `if`) dobra sozinha; dobrar a de fora esconde as duas.
    const interna = Number(focoDepois[0]);
    await page.keyboard.press('ArrowLeft');
    const so = (await linhasVis()).length;
    check(`${largura}: a regiao interna (linha ${interna}) dobra sozinha`, so < antes.length && so > antes.length - N, `${so} de ${antes.length}`);

    // Estado por arquivo: o yaml nao herda; voltar ao main.rs recupera a interna dobrada.
    const dobradasMain = (await painel()).dobradas;
    await helix(':open conf.yaml');
    const py = await ate(v => v.arquivo === 'conf.yaml' && v.modo === 'pronto');
    if (py.arquivo !== 'conf.yaml') { await page.screenshot({ path: join(OUT, `ide_dobra_${largura}_depurar.png`) }); console.log(JSON.stringify(await page.evaluate(() => [...document.querySelectorAll('#ideStatus, #ideAbas')].map(e => e.textContent)))); }
    check(`${largura}: conf.yaml: reserva por indentacao, dita no painel, sem regiao dobrada herdada`, py.origem === 'indentacao' && /indentação/.test(py.estado) && py.dobradas === '' && py.regioes === '2', `${JSON.stringify(py)} | ${await page.textContent('#termEspelho')}`);
    await helix(':open src/main.rs');
    const volta = await ate(v => v.arquivo === 'src/main.rs' && v.modo === 'pronto');
    check(`${largura}: de volta ao main.rs, a regiao dobrada volta (estado por arquivo)`, volta.dobradas === dobradasMain && dobradasMain === String(interna), `${volta.dobradas} vs ${dobradasMain}`);
    // Dobrar tudo / desdobrar tudo.
    await page.click('#leituraDobrarTudo');
    const tudo = (await linhasVis()).length;
    await page.click('#leituraDesdobrarTudo');
    check(`${largura}: DOBRAR TUDO esconde as duas funcoes e DESDOBRAR TUDO devolve`, tudo <= antes.length - N - 1 && (await linhasVis()).length === antes.length, `${tudo} / ${antes.length}`);
    check(`${largura}: sem erro de JavaScript nem violacao de CSP`, erros.length === 0, erros.join(' | '));
    await helix(':qa!');
    await ctx.close();
  }
  await browser.close();
} catch (e) {
  falhou = e;
} finally {
  agente.kill('SIGTERM');
  rmSync(D, { recursive: true, force: true });
}
if (falhou) { console.log('FALHA', falhou.stack || falhou.message); console.log(log.agente.slice(-1500)); checagens.push({ nome: 'excecao', ok: false, detalhe: String(falhou.message) }); }
const passou = checagens.filter(c => c.ok).length;
writeFileSync(join(OUT, 'ide_dobra.json'), JSON.stringify({ roteiro: 'tests/desktop/ide_dobra.mjs', medido_em: new Date().toISOString(), placar: `${passou}/${checagens.length}`, ok: passou === checagens.length && checagens.length > 0, checagens, capturas }, null, 1));
console.log(`placar: ${passou}/${checagens.length}`);
process.exit(checagens.length && passou === checagens.length ? 0 : 1);
