/* A tela de Cluster, exercitada contra um cluster VIVO de tres nos -- frente
 * F2, pedidos 208 (a tela), 217 (escalonar a quente) e 218 (o bloco `cluster`
 * na resposta de `config`).
 *
 * # Por que ela nao entra na `bateria.mjs`
 *
 * A bateria sobe UM phxsqld sem bloco `cluster`, e a tela de Cluster desenha,
 * nesse caso, so a nota «este servidor nao esta em cluster» -- sem botao
 * nenhum. Os quatro botoes dela nascem apenas com o bloco `cluster` no
 * `config.json`, e por isso a bateria nao consegue clica-los: nao ha o que
 * clicar. E este script que os exercita, contra tres servidores de verdade, e
 * e ele que a dispensa do `conferidor_botoes.rs` cita.
 *
 * # O que ele prova, e nao so fotografa
 *
 *   1. a tela LE o cluster -- o que so e possivel desde o 218, porque antes o
 *      `config` nem trazia a chave;
 *   2. `#btClAdd` acrescenta um quarto no A QUENTE: o pulso dele passa a ser
 *      aceito e ele aparece VIVO, sem ninguem reiniciar nada;
 *   3. `#btClQuorum` grava o quorum minimo, e a tela continua dizendo que ele
 *      NAO e imposto -- campo que finge efeito e pior que campo ausente;
 *   4. `#btClDel` tira o no de novo, e a lista volta a tres;
 *   5. `#btClVer` repinta.
 *
 *     node testes-web/capturas-cluster.mjs
 *
 * Grava em docs/dossie/capturas/cluster-*.png, nos dois temas. Derruba cada
 * servidor pelo PID que guardou -- nunca por `pkill -f`, que pegaria o
 * phxsqld do vizinho. */
import { chromium } from '/opt/node22/lib/node_modules/playwright/index.mjs';
import { mkdirSync, mkdtempSync, rmSync, writeFileSync, readFileSync } from 'node:fs';
import { spawn, spawnSync } from 'node:child_process';
import { tmpdir } from 'node:os';
import { connect } from 'node:net';
import { join, resolve } from 'node:path';

const RAIZ = resolve(new URL('.', import.meta.url).pathname, '..');
const SAIDA = join(RAIZ, 'docs/dossie/capturas');
const phxsqld = join(RAIZ, 'target/release/phxsqld');

const USUARIO = 'adm', SENHA = 'segredo1', TOKEN = 'f2cluster';
// Faixa 7200-7299, da frente F2. O web so existe no no1 -- e a tela dele que
// manda no cluster inteiro.
const PORTAS = { no1: 7200, no2: 7202, no3: 7204, no4: 7206 };
const PRIORIDADE = { no1: 3, no2: 2, no3: 1, no4: 0 };
const PORTA_WEB = 7201;
const JANELA_S = 10;

const diz = (...a) => console.log(...a);
const dormir = ms => new Promise(r => setTimeout(r, ms));
const vivos = [];

function hashDaSenha(senha) {
  const r = spawnSync(phxsqld, ['--senha'], { input: senha, encoding: 'utf8' });
  const m = /"senha_hash": "([^"]+)"/.exec(r.stdout || '');
  if (!m) throw new Error(`phxsqld --senha nao devolveu o hash: ${r.stdout}${r.stderr}`);
  return m[1];
}

async function esperarPorta(porta, prazoMs = 20000) {
  const fim = Date.now() + prazoMs;
  while (Date.now() < fim) {
    const abriu = await new Promise(r => {
      const s = connect({ host: '127.0.0.1', port: porta }, () => { s.destroy(); r(true); });
      s.on('error', () => r(false));
      s.setTimeout(500, () => { s.destroy(); r(false); });
    });
    if (abriu) return true;
    await dormir(150);
  }
  return false;
}

function configDe(dir, nome, h, membros) {
  const c = {
    base: join(dir, nome, 'dados'),
    bind: `127.0.0.1:${PORTAS[nome]}`,
    token: TOKEN,
    max_linhas: 1000,
    replicacao: {
      papel: nome === 'no1' ? 'source' : 'replica',
      id_servidor: nome,
      imagem_da_linha: true,
    },
    usuarios: [{
      id: 10, nome: 'Adriano Boller', login: USUARIO,
      senha_hash: h, supervisor: true, ativo: true, bases: {},
    }],
    cluster: {
      id: nome,
      prioridade: PRIORIDADE[nome],
      janela_inatividade_s: JANELA_S,
      pulso_s: 1,
      token: TOKEN,
      usuario: USUARIO,
      senha_hash: h,
      // O campo do 208: guardado e AINDA NAO imposto. Ele nasce aqui para a
      // captura mostrar o valor de verdade, e nao um campo vazio.
      quorum_minimo: 2,
      nos: membros.map(n => ({ id: n, endereco: '127.0.0.1', porta: PORTAS[n] })),
    },
  };
  if (nome === 'no1') c.web = { ligado: true, bind: `127.0.0.1:${PORTA_WEB}`, sessao_minutos: 60 };
  else c.somente_leitura = true;
  return c;
}

async function subir(dir, nome, h, membros) {
  const d = join(dir, nome);
  mkdirSync(d, { recursive: true });
  const caminho = join(d, 'config.json');
  writeFileSync(caminho, JSON.stringify(configDe(dir, nome, h, membros), null, 2));
  const proc = spawn(phxsqld, ['--config', caminho], { cwd: d, stdio: ['ignore', 'pipe', 'pipe'] });
  const saida = [];
  proc.stdout.on('data', x => saida.push(String(x)));
  proc.stderr.on('data', x => saida.push(String(x)));
  let morreu = null;
  proc.on('exit', c => { morreu = c; });
  vivos.push(proc);
  if (!(await esperarPorta(PORTAS[nome]))) {
    throw new Error(`${nome}: a porta ${PORTAS[nome]} nao abriu (saida=${morreu}):\n${saida.join('')}`);
  }
  diz(`  ${nome} pid ${proc.pid} na ${PORTAS[nome]}`);
  return { proc, caminho };
}

function derrubarTodos() {
  for (const p of vivos) {
    try { process.kill(p.pid, 'SIGTERM'); } catch { /* ja morreu */ }
  }
}

async function api(page, op, params = {}) {
  return await page.evaluate(([o, p]) => api(o, p), [op, params]);
}

async function entrar(page, url) {
  await page.goto(url, { waitUntil: 'domcontentloaded' });
  await page.waitForSelector('#btEntrar');
  await page.waitForFunction(() => typeof est === 'object' && est.demo === false, { timeout: 20000 });
  await page.fill('#u', USUARIO);
  await page.fill('#s', SENHA);
  await page.fill('#t', TOKEN);
  await page.click('#btEntrar');
  await page.waitForSelector('#app.ativo[data-pronto="1"]', { timeout: 20000 });
}

/* ------------------------------------------------------------------ main */

const dir = mkdtempSync(join(tmpdir(), `phx-f2-${process.pid}-`));
mkdirSync(SAIDA, { recursive: true });
const h = hashDaSenha(SENHA);
const tres = ['no1', 'no2', 'no3'];
const relato = { passos: [] };
const marcar = (nome, ok, detalhe) => {
  relato.passos.push({ nome, ok, detalhe });
  diz(`  ${ok ? 'ok ' : 'FALHOU'} ${nome}${detalhe ? ' — ' + detalhe : ''}`);
};

diz('sobe o cluster de tres nos');
const arquivos = {};
for (const n of tres) arquivos[n] = (await subir(dir, n, h, tres)).caminho;
if (!(await esperarPorta(PORTA_WEB))) throw new Error('a porta web nao abriu');

const navegador = await chromium.launch({ args: ['--no-sandbox'] });
try {
  for (const tema of ['escuro', 'claro']) {
    diz(`\n── tema ${tema} ──`);
    const ctx = await navegador.newContext({ viewport: { width: 1440, height: 900 }, deviceScaleFactor: 1 });
    await ctx.addInitScript(t => { try { localStorage.setItem('phxsql-tema', t); } catch { /* privado */ } }, tema);
    const page = await ctx.newPage();
    const erros = [];
    page.on('pageerror', e => erros.push(e.message || String(e)));
    await entrar(page, `http://127.0.0.1:${PORTA_WEB}/`);

    // Espera o pulso circular: uma captura tirada antes dele mostraria os
    // outros dois nos como «nunca pulsou», que e verdade por um segundo e
    // mentira sobre o cluster.
    await page.waitForFunction(async () => {
      const e = await api('cluster_estado');
      return e.nos.filter(n => n.vivo).length === 3;
    }, { timeout: 30000 }).catch(() => diz('  ⚠ os tres nao ficaram vivos a tempo'));

    await page.evaluate(() => verCluster());
    await page.waitForSelector('#btClAdd', { timeout: 10000 });
    await dormir(400);
    await page.evaluate(() => { document.querySelector('#painel').scrollTop = 0; });
    await page.screenshot({ path: join(SAIDA, `cluster-${tema}.png`) });
    diz(`  ✓ cluster-${tema}.png`);
    // A metade de baixo -- o escalonamento a quente e o campo do quorum, que
    // e o que os pedidos 217 e 208 pedem para ver.
    await page.evaluate(() => {
      const p = document.querySelector('#painel');
      p.scrollTop = p.scrollHeight;
    });
    await dormir(250);
    await page.screenshot({ path: join(SAIDA, `cluster-acoes-${tema}.png`) });
    diz(`  ✓ cluster-acoes-${tema}.png`);

    if (tema === 'escuro') {
      // (1) a tela LE o cluster -- so possivel desde o 218.
      const cfg = await api(page, 'config');
      marcar('o bloco `cluster` chega na resposta de `config` (218)',
        !!(cfg.cluster && (cfg.cluster.nos || []).length === 3),
        `nos=${cfg.cluster ? (cfg.cluster.nos || []).length : 'ausente'}`);
      marcar('e o token e o hash do cluster NAO chegam junto',
        !JSON.stringify(cfg.cluster).includes(TOKEN)
        && !JSON.stringify(cfg.cluster).includes('pbkdf2'),
        JSON.stringify(cfg.cluster).slice(0, 90));

      // (2) o quarto no, A QUENTE, pelo botao da tela.
      await subir(dir, 'no4', h, [...tres, 'no4']);
      await page.fill('#clNovoId', 'no4');
      await page.fill('#clNovoEnd', '127.0.0.1');
      await page.fill('#clNovaPorta', String(PORTAS.no4));
      await page.click('#btClAdd');
      await page.waitForSelector('#btClAdd', { timeout: 10000 });
      await dormir(300);
      const t0 = Date.now();
      const virou = await page.waitForFunction(async () => {
        const e = await api('cluster_estado');
        const n = e.nos.find(x => x.id === 'no4');
        return !!(n && n.vivo);
      }, { timeout: 30000 }).then(() => true).catch(() => false);
      marcar('#btClAdd acrescenta o no4 A QUENTE e ele aparece VIVO', virou,
        `${Date.now() - t0} ms depois do clique`);
      const gravado = JSON.parse(readFileSync(arquivos.no2, 'utf8')).cluster.nos.map(n => n.id);
      marcar('a ordem foi PROPAGADA e o no2 gravou os quatro no config.json',
        gravado.length === 4, gravado.join(','));

      // (3) o quorum: grava, e continua dizendo que nao e imposto.
      await page.fill('#clQuorum', '3');
      await page.click('#btClQuorum');
      await page.waitForSelector('#clQuorum', { timeout: 10000 });
      await dormir(400);
      const noArquivo = JSON.parse(readFileSync(arquivos.no1, 'utf8')).cluster.quorum_minimo;
      const cfg2 = await api(page, 'config');
      marcar('#btClQuorum grava o quorum minimo no config.json',
        noArquivo === 3, `config.json=${noArquivo}`);
      marcar('e o SERVIDOR continua dizendo que ele nao e imposto',
        cfg2.cluster.quorum_imposto === false, `quorum_imposto=${cfg2.cluster.quorum_imposto}`);
      const dom = await page.evaluate(() => document.querySelector('#painel').innerText);
      marcar('a tela diz isso com todas as letras', /NÃO é imposto/.test(dom),
        (dom.match(/.{0,40}NÃO é imposto.{0,20}/) || ['(nao achei a frase)'])[0]);

      // (4) o remover, pelo botao -- com o `confirm()` aceito.
      page.once('dialog', d => d.accept());
      await page.selectOption('#clSaiId', 'no4');
      await page.click('#btClDel');
      await page.waitForSelector('#btClAdd', { timeout: 10000 });
      await dormir(500);
      const depois = await api(page, 'cluster_estado');
      marcar('#btClDel tira o no4 e a lista volta a tres',
        depois.nos.length === 3, depois.nos.map(n => n.id).join(','));

      // (5) o repintar.
      await page.click('#btClVer');
      await page.waitForSelector('#gradeCluster', { timeout: 10000 });
      marcar('#btClVer repinta a tela', true, '');
    }

    if (erros.length) {
      marcar('nenhum erro de pagina', false, erros.slice(0, 4).join(' | '));
    } else if (tema === 'claro') {
      marcar('nenhum erro de pagina nos dois temas', true, '');
    }
    await ctx.close();
  }
  writeFileSync(join(RAIZ, 'testes-web/.capturas-cluster-relato.json'),
    JSON.stringify(relato, null, 2));
  const falhas = relato.passos.filter(p => !p.ok);
  diz(`\n${relato.passos.length - falhas.length} de ${relato.passos.length} passos ok`);
  if (falhas.length) process.exitCode = 1;
} finally {
  await navegador.close();
  derrubarTodos();
  await dormir(1500);
  try { rmSync(dir, { recursive: true, force: true }); } catch { /* tanto faz */ }
  diz('servidores derrubados pelo PID, diretorio temporario removido.');
}
