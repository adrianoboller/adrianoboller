// O assistente e o no `politica` da tela Fluxos EXERCITADOS contra o agente REAL (`phxclaw
// servir`), com o modelo do agente falando com um Ollama falso de roteiro: a descricao vai
// pela tela, a 1a resposta o motor recusa, a 2a (um fluxo com politica) vira RASCUNHO na pasta,
// a lista recarrega e o rascunho abre no editor -- o no POLITICA desenhado, as portas
// «aprovado»/«reprovado» nomeadas nas ligacoes, nada publicado no disco. Depois, o roteiro so
// com respostas invalidas: a tela avisa (role=alert) e nada e gravado. Nas larguras 1280 e
// 400, com o contraste da caixa e do botao medido e a descricao sem text-transform.
//
// Uso: PLAYWRIGHT_BROWSERS_PATH=/opt/pw-browsers node tests/desktop/ui_fluxos_assistente.mjs [BINARIO] [--ui DIR]
// Resultado em tests/desktop/out/ui_fluxos_assistente.json; capturas
// ui_fluxos_assistente_{1280,400}.png. RED medido: copia da UI sem o `politica` nos TIPOS do
// fluxos.js -- o no sai como «?» e a checagem do desenho cai (placar 11/12).
import { createRequire } from 'node:module';
import { spawn } from 'node:child_process';
import { createServer } from 'node:http';
import { mkdtempSync, writeFileSync, readdirSync, existsSync, mkdirSync, rmSync } from 'node:fs';
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
  checagens.push({ nome, ok: !!ok, detalhe: String(detalhe).slice(0, 400) });
  console.log(`${ok ? 'ok   ' : 'FALHA '}${nome}${detalhe ? ` :: ${String(detalhe).slice(0, 400)}` : ''}`);
}
const esperar = ms => new Promise(r => setTimeout(r, ms));

// O fluxo que o «modelo» devolve na 2a tentativa: a politica com as duas portas usadas.
const VALIDO = {
  nome: 'triagem-com-guarda',
  passos: [
    { id: 'entrada_dados', ferramenta: 'read_file', args: { path: 'mensagens.json' } },
    { id: 'guarda', depende: ['entrada_dados'], politica: { credenciais: true, injecao: true, pii: ['cpf', 'email'] } },
    { id: 'responde', depende: ['guarda:aprovado'], tarefa: 'responda {{guarda}}' },
    { id: 'avisa', depende: ['guarda:reprovado'], ferramenta: 'write_file', args: { path: 'reprovados.json', content: '{{guarda}}' } },
  ],
};
const INVALIDO = JSON.stringify({ nome: 'triagem-com-guarda', passos: [{ id: 'a', tarefa: 'use {{b}}' }] });
let roteiro = [INVALIDO, JSON.stringify(VALIDO)];
const vistos = [];
const ollama = createServer((req, res) => {
  let corpo = '';
  req.on('data', c => { corpo += c; });
  req.on('end', () => {
    let conteudo = roteiro[0] ?? INVALIDO;
    if (req.url.includes('/api/chat')) { vistos.push(corpo); conteudo = roteiro.shift() ?? INVALIDO; }
    const r = JSON.stringify({ model: 'falso', message: { role: 'assistant', content: conteudo }, prompt_eval_count: 10, eval_count: 5, done: true });
    res.writeHead(200, { 'content-type': 'application/json' });
    res.end(r);
  });
});
await new Promise(r => ollama.listen(0, '127.0.0.1', r));
const OLLAMA = `http://127.0.0.1:${ollama.address().port}`;

const D = mkdtempSync(join(tmpdir(), 'phx-ui-assistente-'));
const PASTA = join(D, 'agente');
const FLUXOS = join(PASTA, 'fluxos');
mkdirSync(FLUXOS, { recursive: true });
writeFileSync(join(FLUXOS, 'existente.json'), '{"nome": "existente", "passos": [{"id": "a", "tarefa": "x"}]}\n');
const token = 'token-ui-assistente-' + Date.now();
writeFileSync(join(PASTA, 'api.token'), token);
const porta = 18700 + Math.floor(Math.random() * 900);
const log = { agente: '' };
const agente = spawn(BIN, ['servir', '--porta', String(porta), '--pasta', PASTA], {
  env: { PATH: process.env.PATH, HOME: D, PHXCLAW_PROJETO: D, ...(UI_DIR ? { PHXCLAW_UI_DIR: UI_DIR } : {}), PHXCLAW_MODELO: 'ollama:falso', OLLAMA_HOST: OLLAMA, PHXCLAW_API_TOKEN: token },
  stdio: ['ignore', 'pipe', 'pipe'],
});
agente.stdout.on('data', b => { log.agente += b; });
agente.stderr.on('data', b => { log.agente += b; });
const ORIG = `http://127.0.0.1:${porta}`;
const capturas = [];
let falhou = null;

// Contraste WCAG do texto do elemento contra o fundo opaco mais proximo.
const CONTRASTE = sel => `(() => {
  const rgb = s => { const m = String(s).match(/[\\d.]+/g) || [0, 0, 0]; return m.slice(0, 3).map(Number); };
  const lum = c => { const [r, g, b] = c.map(v => { v /= 255; return v <= 0.03928 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4; }); return 0.2126 * r + 0.7152 * g + 0.0722 * b; };
  const fundo = e => { for (let n = e; n; n = n.parentElement) { const c = getComputedStyle(n).backgroundColor; if (c && !/rgba\\(.*, 0\\)$/.test(c) && c !== 'transparent') return c; } return getComputedStyle(document.body).backgroundColor; };
  const e = document.querySelector(${JSON.stringify(sel)});
  const [x, y] = [lum(rgb(getComputedStyle(e).color)), lum(rgb(fundo(e)))].sort((p, q) => q - p);
  return +((x + 0.05) / (y + 0.05)).toFixed(2);
})()`;

try {
  let vivo = false;
  for (let i = 0; i < 100 && !vivo; i++) {
    await esperar(200);
    try { vivo = (await fetch(`${ORIG}/health`)).ok; } catch {}
  }
  check('o agente real sobe e serve a UI', vivo, log.agente.split('\n').slice(-3).join(' | '));
  const browser = await chromium.launch();
  const erros = [];
  for (const largura of [1280, 400]) {
    const ctx = await browser.newContext({ viewport: { width: largura, height: largura < 640 ? 860 : 900 }, locale: 'pt-BR', hasTouch: largura < 640 });
    await ctx.addInitScript(t => { try { localStorage.setItem('phxclaw.token', t); } catch {} }, token);
    const page = await ctx.newPage();
    page.on('pageerror', e => erros.push(`${largura}: ${e}`));
    await page.goto(`${ORIG}/?tela=fluxos`);
    await page.waitForSelector('.fluxo-item[data-arquivo="existente.json"]');
    const tt = await page.$eval('#fluxosDescricao', e => getComputedStyle(e).textTransform);
    check(`${largura}: a descricao e escrita como digitada (sem text-transform)`, tt === 'none', tt);
    const rCaixa = await page.evaluate(CONTRASTE('#fluxosDescricao'));
    const rBotao = await page.evaluate(CONTRASTE('#fluxosCriar'));
    check(`${largura}: contraste da caixa e do botao >= 4,5:1`, rCaixa >= 4.5 && rBotao >= 4.5, `caixa=${rCaixa} botao=${rBotao}`);
    if (largura === 1280) {
      await page.fill('#fluxosDescricao', 'triar as mensagens: as que tem CPF ou credencial vao para um arquivo, as outras recebem resposta');
      await page.click('#fluxosCriar');
      await page.waitForSelector('.fluxo-item[data-arquivo="triagem-com-guarda.json"]', { timeout: 20000 }).catch(async e => {
        throw new Error(`${e.message} :: status=${await page.$eval('#fluxosStatus', x => x.textContent)} :: chamadas ao modelo=${vistos.length}`);
      });
      await page.waitForFunction(() => document.getElementById('fluxosArquivo').textContent === 'triagem-com-guarda.json', null, { timeout: 20000 });
      await page.waitForTimeout(400);
      const st = await page.$eval('#fluxosStatus', e => e.textContent);
      check('o rascunho nasce, abre no editor e o aviso diz as tentativas', /Rascunho triagem-com-guarda\.json/.test(st) && /2 tentativa/.test(st), st);
      check('a 2a chamada ao modelo levou o erro do motor', vistos.length === 2 && /sem declarar/.test(vistos[1]), vistos.length);
      const g = await page.evaluate(() => ({
        tipos: [...document.querySelectorAll('#fluxosSvg .no-tipo')].map(t => t.textContent),
        portas: [...document.querySelectorAll('#fluxosSvg .aresta-porta')].map(t => t.textContent).sort(),
        veredito: document.getElementById('fluxosVeredito').dataset.valido,
      }));
      check('o no POLITICA e desenhado, com as portas aprovado/reprovado nas ligacoes', g.tipos.includes('POLÍTICA') && g.portas.join(',') === 'aprovado,reprovado' && g.veredito === '1', JSON.stringify(g));
      check('nada publicado: sem pasta de versoes ao lado do rascunho', existsSync(join(FLUXOS, 'triagem-com-guarda.json')) && !existsSync(join(FLUXOS, '.triagem-com-guarda.versoes')), readdirSync(FLUXOS).join(','));
      await page.screenshot({ path: join(OUT, 'ui_fluxos_assistente_1280.png') });
      capturas.push('ui_fluxos_assistente_1280.png');
      // so respostas invalidas: a tela avisa e nada nasce
      roteiro = [INVALIDO, INVALIDO, INVALIDO];
      const antes = readdirSync(FLUXOS).sort().join(',');
      await page.fill('#fluxosDescricao', 'outro fluxo');
      await page.click('#fluxosCriar');
      await page.waitForFunction(() => document.getElementById('fluxosStatus').getAttribute('role') === 'alert', null, { timeout: 20000 });
      const st2 = await page.$eval('#fluxosStatus', e => e.textContent);
      check('sem fluxo valido: alerta dizendo, nada gravado, botao de volta', /não chegou a um fluxo válido/.test(st2) && readdirSync(FLUXOS).sort().join(',') === antes && !(await page.$eval('#fluxosCriar', b => b.disabled)), st2);
    } else {
      const lado = await page.evaluate(() => { const c = document.getElementById('fluxosDescricao').getBoundingClientRect(); const b = document.getElementById('fluxosCriar').getBoundingClientRect(); return { caixa: c.width, botaoAbaixo: b.top >= c.bottom - 1, cabe: c.right <= window.innerWidth && b.right <= window.innerWidth }; });
      check('400: a caixa ocupa a linha e o botao vai embaixo, sem estourar a largura', lado.botaoAbaixo && lado.cabe && lado.caixa > 300, JSON.stringify(lado));
      await page.screenshot({ path: join(OUT, 'ui_fluxos_assistente_400.png') });
      capturas.push('ui_fluxos_assistente_400.png');
    }
    await ctx.close();
  }
  check('nenhum erro de pagina', !erros.length, erros.join(' | '));
  await browser.close();
} catch (e) {
  falhou = e;
  check('o roteiro terminou sem excecao', false, e.stack || e);
} finally {
  agente.kill('SIGTERM');
  ollama.close();
  rmSync(D, { recursive: true, force: true });
}
const passou = checagens.filter(c => c.ok).length;
console.log(`placar: ${passou}/${checagens.length}`);
writeFileSync(join(OUT, 'ui_fluxos_assistente.json'), JSON.stringify({ roteiro: 'tests/desktop/ui_fluxos_assistente.mjs', medido_em: new Date().toISOString(), placar: `${passou}/${checagens.length}`, ok: passou === checagens.length && checagens.length > 0 && !falhou, checagens, capturas }, null, 1));
process.exit(checagens.length && passou === checagens.length && !falhou ? 0 : 1);
