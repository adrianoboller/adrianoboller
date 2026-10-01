// Prova do controle remoto com TRES processos reais: a ponte (`phxclaw ponte`), o agente
// (`phxclaw servir --ponte ... --sem-porta`, que so conecta de SAIDA) e o cliente, um
// Chromium de celular aberto na ponte. Confere tambem que o aplicativo e instalavel pelo
// proprio Chromium (Page.getInstallabilityErrors vazio) e que o service worker controla
// a pagina.
//
// O modelo e um Ollama falso dentro deste script: pergunta a cor (ask_user) e responde
// com final_answer. O que se prova e o caminho -- tela -> ponte -> fio de saida -> API do
// agente -> motor --, nao a inteligencia do modelo.
//
// Uso: PLAYWRIGHT_BROWSERS_PATH=/opt/pw-browsers node tests/desktop/pwa_ponte.mjs [BINARIO]
// Sai 0 so se todas as checagens passarem; capturas em tests/desktop/out/pwa_*.png.
import { createRequire } from 'node:module';
import { spawn, execFileSync } from 'node:child_process';
import { mkdtempSync, writeFileSync, readFileSync, mkdirSync, rmSync, readdirSync, readlinkSync, existsSync } from 'node:fs';
import { createServer } from 'node:http';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { randomUUID } from 'node:crypto';

const require = createRequire(import.meta.url);
const { chromium, devices } = require('/opt/node22/lib/node_modules/playwright');
const AQUI = dirname(fileURLToPath(import.meta.url));
const RAIZ = resolve(AQUI, '../..');
const BIN = resolve(process.argv[2] || join(RAIZ, 'target/debug/phxclaw'));
const OUT = join(AQUI, 'out');
mkdirSync(OUT, { recursive: true });
const D = mkdtempSync(join(tmpdir(), 'phx-pwa-ponte-'));

const checagens = [];
function check(nome, ok, detalhe = '') {
  checagens.push(!!ok);
  console.log(`${ok ? 'ok   ' : 'FALHA '}${nome}${detalhe ? ` :: ${detalhe}` : ''}`);
}
const esperar = ms => new Promise(r => setTimeout(r, ms));
const porta = () => new Promise(r => { const s = createServer(); s.listen(0, '127.0.0.1', () => { const p = s.address().port; s.close(() => r(p)); }); });

// Ollama falso: sem resultado de ferramenta ainda -> pergunta; com -> responde.
const ollama = createServer((req, res) => {
  let corpo = '';
  req.on('data', c => { corpo += c; });
  req.on('end', () => {
    const msgs = JSON.parse(corpo).messages || [];
    const resposta = msgs.filter(m => m.role === 'tool').pop()?.content;
    const call = resposta === undefined
      ? { function: { name: 'ask_user', arguments: { question: 'Qual cor?' } } }
      : { function: { name: 'final_answer', arguments: { answer: `cor escolhida: ${resposta}` } } };
    res.setHeader('Content-Type', 'application/json');
    res.end(JSON.stringify({ message: { role: 'assistant', content: '', tool_calls: [call] }, prompt_eval_count: 1, eval_count: 1, done: true }));
  });
});
await new Promise(r => ollama.listen(0, '127.0.0.1', r));

const sh = args => execFileSync('openssl', args, { cwd: D, stdio: 'ignore' });
writeFileSync(join(D, 'ext.cnf'), 'basicConstraints=CA:FALSE\nsubjectAltName=DNS:localhost\nextendedKeyUsage=serverAuth\n');
sh(['req', '-x509', '-newkey', 'ec', '-pkeyopt', 'ec_paramgen_curve:P-256', '-nodes', '-days', '1', '-subj', '/CN=CA ponte', '-keyout', 'ca.key', '-out', 'ca.pem']);
sh(['req', '-newkey', 'ec', '-pkeyopt', 'ec_paramgen_curve:P-256', '-nodes', '-subj', '/CN=localhost', '-keyout', 'srv.key', '-out', 'srv.csr']);
sh(['x509', '-req', '-in', 'srv.csr', '-CA', 'ca.pem', '-CAkey', 'ca.key', '-CAcreateserial', '-days', '1', '-extfile', 'ext.cnf', '-out', 'srv.pem']);
const tenant = '01920000-0000-7000-8000-000000000001';
const tokenPareamento = `pareamento-${randomUUID()}`;
const tokenPonte = `cliente-${randomUUID()}`;
writeFileSync(join(D, 'tokens'), `${tenant} ${tokenPareamento} agent.http\n`);

const [pHttp, pWss] = [await porta(), await porta()];
const filhos = [];
const log = {};
function subir(nome, args, env) {
  const f = spawn(BIN, args, { env: { PATH: process.env.PATH, HOME: D, ...env }, stdio: ['ignore', 'pipe', 'pipe'] });
  log[nome] = '';
  f.stdout.on('data', b => { log[nome] += b; });
  f.stderr.on('data', b => { log[nome] += b; });
  filhos.push(f);
  return f;
}
let falhou = null;
try {
  subir('ponte', ['ponte', '--porta', String(pHttp), '--porta-wss', String(pWss), '--cert', join(D, 'srv.pem'), '--chave', join(D, 'srv.key'), '--tokens', join(D, 'tokens'), '--pasta', join(D, 'ponte')],
    { PHXCLAW_PONTE_TOKEN: tokenPonte });
  await esperar(800);
  const agente = subir('agente', ['servir', '--sem-porta', '--ponte', `wss://localhost:${pWss}/`, '--ponte-ca', join(D, 'ca.pem'), '--ponte-tenant', tenant, '--pasta', join(D, 'agente')],
    { PHXCLAW_ENROLLMENT_TOKEN: tokenPareamento, OLLAMA_HOST: `http://127.0.0.1:${ollama.address().port}`, PHXCLAW_MODELO: 'ollama:falso', PHXCLAW_LSP: '0', PHXCLAW_PROJETO: D });
  let ligado = false;
  for (let i = 0; i < 100 && !ligado; i++) { await esperar(200); ligado = /sessao .* aberta/.test(log.agente); }
  check('o agente se ligou a ponte por conexao de saida', ligado, log.agente.split('\n').slice(-3).join(' | '));
  // Nenhuma porta TCP ouvindo no processo do agente: so a conexao que ele abriu.
  // Lido do /proc (sem `ss` no conteiner): os sockets do processo cruzados com os
  // sockets em LISTEN (estado 0A) de /proc/net/tcp e tcp6.
  const meus = new Set(readdirSync(`/proc/${agente.pid}/fd`).map(f => { try { return readlinkSync(`/proc/${agente.pid}/fd/${f}`); } catch { return ''; } })
    .map(l => /^socket:\[(\d+)\]$/.exec(l)?.[1]).filter(Boolean));
  const emEscuta = ['tcp', 'tcp6'].filter(t => existsSync(`/proc/net/${t}`)).flatMap(t => readFileSync(`/proc/net/${t}`, 'utf8').split('\n').slice(1))
    .map(l => l.trim().split(/\s+/)).filter(c => c[3] === '0A').map(c => c[9]);
  const ouvindo = emEscuta.filter(i => meus.has(i));
  const conectados = meus.size;
  check('o agente nao abre porta de entrada (nenhum socket em LISTEN)', ouvindo.length === 0 && conectados > 0, `listen=${ouvindo.length} sockets=${conectados}`);

  const browser = await chromium.launch();
  const ctx = await browser.newContext({ ...devices['Pixel 7'] });
  const page = await ctx.newPage();
  await page.goto(`http://localhost:${pHttp}/?tela=tarefas`);
  await page.evaluate(() => navigator.serviceWorker.ready);
  await page.reload();
  const controlado = await page.evaluate(() => !!navigator.serviceWorker.controller);
  check('o service worker controla a pagina servida pela ponte', controlado);
  const cdp = await ctx.newCDPSession(page);
  const inst = await cdp.send('Page.getInstallabilityErrors');
  check('Chromium: aplicativo instalavel (sem erro de instalabilidade)', inst.installabilityErrors.length === 0, JSON.stringify(inst.installabilityErrors));
  const man = await cdp.send('Page.getAppManifest');
  check('Chromium leu o manifesto sem erro', man.errors.length === 0 && /PhxClaw/.test(man.data || ''), JSON.stringify(man.errors));

  await page.waitForSelector('#tela-tarefas:not([hidden])');
  await page.fill('#tarefasTokenCampo', tokenPonte);
  await page.click('#tarefasToken button');
  await page.fill('#tarefasObjetivo', 'escolha uma cor');
  await page.click('#tarefasNova button');
  await page.waitForSelector('.tarefa-pergunta', { timeout: 30000 });
  const pergunta = await page.textContent('.tarefa-questao');
  check('a pergunta da tarefa chega ao celular pela ponte', pergunta === 'Qual cor?', pergunta);
  await page.screenshot({ path: join(OUT, 'pwa_pergunta.png') });
  await page.fill('#tarefasResposta', 'azul');
  await page.click('.tarefa-pergunta button');
  await page.waitForSelector('.tarefa-resposta', { timeout: 30000 });
  const resposta = await page.textContent('.tarefa-resposta');
  check('a resposta (rota /answer) volta ao agente e a tarefa conclui', /cor escolhida: .*azul/.test(resposta), resposta);
  // A lista e uma grade (phx-grid): o status mora na celula da coluna de tag «status».
  const estadoItem = await page.textContent('#tarefasLista td[data-tag="status"]');
  check('a lista mostra o estado pela fabrica de idiomas', estadoItem === 'CONCLUÍDA', estadoItem);
  await page.screenshot({ path: join(OUT, 'pwa_concluida.png') });
  // A ponte recusa rota fora do controle remoto, mesmo com o token certo.
  const st = await page.evaluate(async t => (await fetch('./v1/schedules', { headers: { Authorization: `Bearer ${t}` } })).status, tokenPonte);
  check('a ponte recusa rota fora do controle remoto (403)', st === 403, String(st));
  await browser.close();
} catch (e) {
  falhou = e;
} finally {
  for (const f of filhos) f.kill('SIGTERM');
  ollama.close();
  rmSync(D, { recursive: true, force: true });
}
if (falhou) { console.log('FALHA', falhou.message); console.log(log); checagens.push(false); }
const ok = checagens.filter(Boolean).length;
console.log(`placar: ${ok}/${checagens.length}`);
process.exit(ok === checagens.length ? 0 : 1);
