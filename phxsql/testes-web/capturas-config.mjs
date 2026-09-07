/* Capturas da tela de Configurações (gerais do servidor e do banco atual) --
 * frente F8, pedido N) do docs/pdf.
 *
 * Por que existe: o pedido N) do dono e "telas das configuracoes gerais e do
 * banco de dados atual", com exemplo exercitado contra o motor vivo -- nao
 * basta ler o codigo, "Interface so se prova exercitando" (CLAUDE.md). Este
 * script sobe um phxsqld SO dele (faixa 6800-6801, dentro dos 6800-6819 da
 * frente F8), cria um banco com tabela e dado, entra pela tela de login como
 * qualquer pessoa entraria, e fotografa cada SECAO da tela "Gerais do
 * servidor" (GRUPOS_AJUSTE, em crates/phxsql-server/ui/index.html) e a tela
 * "Do banco atual" -- nos dois temas, em 1440x900. De quebra, exercita as
 * duas leis que a tela promete: o campo salvo sobrevive a um F5 (SEGURANCA.md
 * §9), e um segredo (senha do rele de e-mail) nunca chega ao HTML nem a
 * resposta de `config`.
 *
 * Como roda (o binario NAO se compila aqui -- usa o de target/release/):
 *
 *     node testes-web/capturas-config.mjs
 *
 * Grava em docs/dossie/capturas/config-<secao>-<tema>.png e
 * docs/dossie/capturas/banco-atual-<tema>.png. Derruba o servidor pelo PID
 * guardado -- nunca por `pkill -f`, que pegaria o phxsqld do vizinho. */
import { chromium } from '/opt/node22/lib/node_modules/playwright/index.mjs';
import { mkdirSync, mkdtempSync, rmSync, writeFileSync, readFileSync } from 'node:fs';
import { spawn, spawnSync } from 'node:child_process';
import { tmpdir } from 'node:os';
import { connect } from 'node:net';
import { join, resolve } from 'node:path';

const RAIZ = resolve(new URL('.', import.meta.url).pathname, '..');
const SAIDA = join(RAIZ, 'docs/dossie/capturas');
const phxsqld = join(RAIZ, 'target/release/phxsqld');

const USUARIO = 'adm', SENHA = 'segredo1', TOKEN = 'f8config';
const SENHA_RELE = 'MARCA-SEGREDO-RELE-8f2c';   // o que NAO pode aparecer na tela
const SENHA_CIFRA = 'MARCA-SEGREDO-COFRE-9a1d';  // idem, do lado da cifra

const PORTA_DADOS = 6800;
const PORTA_WEB = 6801;

const diz = (...a) => console.log(...a);
const dormir = ms => new Promise(r => setTimeout(r, ms));

/* --------------------------------------------------- o servidor da foto */

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

async function subir(dir) {
  const base = join(dir, 'dados');
  const caminho = join(dir, 'config.json');
  // A cifra e o rele de e-mail entram LIGADOS, com senha de mentira, so para
  // este script poder provar que ela nao volta em texto puro -- nem na tela,
  // nem no JSON que `config` devolve. Sem um segredo de verdade no arquivo, a
  // ausencia dele na resposta nao provaria nada.
  writeFileSync(caminho, JSON.stringify({
    base, bind: `127.0.0.1:${PORTA_DADOS}`, token: TOKEN, max_linhas: 1000,
    web: { ligado: true, bind: `127.0.0.1:${PORTA_WEB}`, sessao_minutos: 60 },
    recursos: { durabilidade: 'sistema', cache_paginas: 512 },
    usuarios: [{
      id: 10, nome: 'Adriano Boller', login: USUARIO,
      senha_hash: hashDaSenha(SENHA), supervisor: true, ativo: true, bases: {},
    }],
    replicacao: { papel: 'isolado' },
    cifra: { ligada: true, senha: SENHA_CIFRA, tabelas: [] },
    alertas: {
      ligado: true, livre_minimo_percentual: 10, livre_minimo_mb: 512,
      email: {
        ligado: true, servidor: '127.0.0.1', porta: 25, de: 'phxsql@local.test',
        para: ['dba@local.test'], usuario: 'phxsql', senha: SENHA_RELE,
        assunto: 'PhxSql', avisar_jobs: false,
      },
    },
  }, null, 2));

  const proc = spawn(phxsqld, ['--config', caminho], { cwd: dir, stdio: ['ignore', 'pipe', 'pipe'] });
  const saida = [];
  proc.stdout.on('data', d => saida.push(String(d)));
  proc.stderr.on('data', d => saida.push(String(d)));
  let morreu = null;
  proc.on('exit', c => { morreu = c; });
  diz(`servidor pid ${proc.pid} — dados ${PORTA_DADOS}, web ${PORTA_WEB}`);

  const matar = () => {
    try { process.kill(proc.pid, 'SIGTERM'); } catch { /* ja morreu */ }
    setTimeout(() => { try { process.kill(proc.pid, 'SIGKILL'); } catch { /* ok */ } }, 4000).unref();
  };
  if (!(await esperarPorta(PORTA_WEB)) || !(await esperarPorta(PORTA_DADOS))) {
    matar();
    throw new Error(`as portas nao abriram (saida=${morreu}):\n${saida.join('')}`);
  }
  if (morreu !== null) {
    throw new Error(`o phxsqld morreu com codigo ${morreu} logo apos abrir a porta:\n${saida.join('')}`);
  }
  return {
    pid: proc.pid,
    url: `http://127.0.0.1:${PORTA_WEB}/`,
    async derrubar() {
      matar();
      for (let i = 0; i < 60 && morreu === null; i++) await dormir(100);
    },
  };
}

/* ------------------------------------------------------------ o cenario */

async function api(page, op, params = {}) {
  return await page.evaluate(([o, p]) => api(o, p), [op, params]);
}

async function popular(page) {
  await api(page, 'criar_database', { database: 'Comercial' }).catch(() => {});
  await api(page, 'criar_tabela', {
    database: 'Comercial', tabela: 'clientes',
    colunas: [
      { nome: 'id', tipo: 'Int4', obrigatoria: true, caption: 'Código' },
      { nome: 'nome', tipo: 'Str(40)', obrigatoria: true, caption: 'Nome' },
      { nome: 'cidade', tipo: 'Str(30)', caption: 'Cidade' },
      { nome: 'uf', tipo: 'Str(2)', caption: 'UF' },
      { nome: 'limite', tipo: 'Decimal(12,2)', caption: 'Limite' },
    ],
    indices: [
      { nome: 'porId', colunas: ['id'], unico: true, primario: true },
      { nome: 'porNome', colunas: ['nome'], nocase: true },
    ],
  }).catch(e => diz('  clientes:', e));
  const linhas = [
    [1, 'Adriano Boller', 'Blumenau', 'SC', '15000.00'],
    [2, 'Maria Souza', 'Joinville', 'SC', '2500.50'],
    [3, 'Carlos Lima', 'Curitiba', 'PR', '900.00'],
  ];
  for (const l of linhas) {
    await api(page, 'inserir', { database: 'Comercial', tabela: 'clientes', valores: l }).catch(() => {});
  }
}

/* ------------------------------------------------------------- a entrada */

async function entrar(page, url) {
  await page.goto(url, { waitUntil: 'domcontentloaded' });
  await page.waitForSelector('#btEntrar');
  await page.waitForFunction(() => typeof est === 'object' && est.demo === false,
    { timeout: 20000 }).catch(() => { throw new Error('a pagina caiu em modo demonstracao'); });
  await page.fill('#u', USUARIO);
  await page.fill('#s', SENHA);
  await page.fill('#t', TOKEN);
  await page.click('#btEntrar');
  await page.waitForSelector('#app.ativo[data-pronto="1"]', { timeout: 20000 });
}

/* -------------------------------------------------- recorte por secao */

/** Acha cada `<h3 class="secao">` filho direto de #painel e o intervalo (por
 * INDICE de filho) que ela ocupa, ate o proximo h3.secao ou o fim de #painel.
 *
 * `#painel` rola por DENTRO de si mesmo (a pagina toda tem 900px fixos --
 * cabecalho, barra e rodape nao rolam), entao `document.documentElement` nunca
 * cresce e um `fullPage` do Playwright nao alcanca secao nenhuma abaixo da
 * dobra. O que funciona e rolar o PAINEL ate a secao e medir DEPOIS de rolar
 * -- por isso aqui so guardamos os indices, e a rolagem+medida de verdade
 * acontece em `capturarSecoes`, secao por secao. */
async function secoesDoPainel(page) {
  return await page.evaluate(() => {
    const painel = document.querySelector('#painel');
    const filhos = [...painel.children];
    const marcos = filhos
      .map((el, i) => ({ i, titulo: el.matches('h3.secao') ? el.textContent.trim() : null }))
      .filter(x => x.titulo !== null);
    return marcos.map((m, k) => ({
      titulo: m.titulo, ini: m.i,
      fim: k + 1 < marcos.length ? marcos[k + 1].i : filhos.length,
    }));
  });
}

/** Nome de arquivo estavel a partir do titulo da secao (acentos fora, minusculo). */
function slug(titulo) {
  return titulo.normalize('NFD').replace(/[̀-ͯ]/g, '')
    .toLowerCase().replace(/[^a-z0-9]+/g, '-').replace(/(^-|-$)/g, '');
}

async function capturarSecoes(page, tema, prefixo) {
  const secoes = await secoesDoPainel(page);
  const nomes = [];
  for (const s of secoes) {
    const arq = `${prefixo}-${slug(s.titulo)}-${tema}.png`;
    // Rola #painel (nao a pagina -- ela nao rola) ate o comeco da secao, e SO
    // ENTAO mede: a rolagem muda o `getBoundingClientRect` de todo mundo.
    const rect = await page.evaluate(({ ini, fim }) => {
      const painel = document.querySelector('#painel');
      const grupo = [...painel.children].slice(ini, fim);
      grupo[0].scrollIntoView({ block: 'start' });
      const rects = grupo.map(e => e.getBoundingClientRect());
      return {
        top: Math.min(...rects.map(r => r.top)),
        bottom: Math.max(...rects.map(r => r.bottom)),
        left: Math.min(...rects.map(r => r.left)),
        right: Math.max(...rects.map(r => r.right)),
      };
    }, s);
    // O clip nao pode passar do que a viewport realmente pintou: uma secao
    // mais alta que os 900px fica cortada na dobra -- e continua rolavel na
    // tela de verdade, exatamente como a pessoa veria sem rolar mais.
    const y = Math.max(0, rect.top);
    const height = Math.min(rect.bottom, 900) - y;
    await page.screenshot({
      path: join(SAIDA, arq),
      clip: { x: Math.max(0, rect.left), y, width: Math.max(1, rect.right - rect.left), height: Math.max(1, height) },
    });
    diz(`  ✓ ${arq}  (${s.titulo})`);
    nomes.push({ titulo: s.titulo, arquivo: arq });
  }
  return nomes;
}

/* ------------------------------------------------------------------ main */

const dir = mkdtempSync(join(tmpdir(), `phx-f8-${process.pid}-`));
mkdirSync(SAIDA, { recursive: true });

const servidor = await subir(dir);
diz(`servidor no ar: ${servidor.url}  (dir=${dir})`);
const navegador = await chromium.launch();
const relato = { secoesGerais: [], exercicioSalvar: null, exercicioSegredo: null };
try {
  for (const tema of ['escuro', 'claro']) {
    diz(`\n── tema ${tema} ──`);
    const ctx = await navegador.newContext({ viewport: { width: 1440, height: 900 }, deviceScaleFactor: 1 });
    await ctx.addInitScript(t => { try { localStorage.setItem('phxsql-tema', t); } catch { /* privado */ } }, tema);
    const page = await ctx.newPage();
    const erros = [];
    page.on('pageerror', e => erros.push(e.message || String(e)));

    await entrar(page, servidor.url);
    if (tema === 'escuro') { diz('  populando…'); await popular(page); }

    // --- tela "Gerais do servidor"
    await page.evaluate(() => verConfigServidor());
    await dormir(500);
    const secoes = await capturarSecoes(page, tema, 'config');
    if (tema === 'escuro') relato.secoesGerais = secoes.map(s => s.titulo);

    // --- tela "Do banco atual"
    await page.evaluate(() => { est.db = 'Comercial'; verConfigBanco('Comercial'); });
    await dormir(500);
    // A tela inteira nao cabe em 900px, e a pagina nao rola (so #painel
    // rola) -- por isso a captura e o PAINEL rolado ao topo, do mesmo jeito
    // que uma pessoa a veria abrindo a tela sem rolar ainda.
    await page.evaluate(() => { document.querySelector('#painel').scrollTop = 0; });
    await page.screenshot({ path: join(SAIDA, `banco-atual-${tema}.png`) });
    diz(`  ✓ banco-atual-${tema}.png`);

    if (erros.length) diz(`  ⚠ pageerror: ${erros.slice(0, 4).join(' | ')}`);
    await ctx.close();
  }

  // ---------------------------------------------------- exercicio: salvar
  // Muda max_linhas pela tela, salva, RECARREGA a pagina do zero (F5 de
  // verdade, nao um redesenho em JS) e confere que o valor persistiu -- e o
  // que o SEGURANCA.md §9 promete (gravacao atomica, campo mostra o gravado).
  {
    const ctx = await navegador.newContext({ viewport: { width: 1440, height: 900 } });
    const page = await ctx.newPage();
    await entrar(page, servidor.url);
    await page.evaluate(() => verConfigServidor());
    await dormir(400);
    const antes = await page.$eval('[data-campo="max_linhas"]', el => el.value);
    const novo = String(Number(antes) + 111);
    await page.fill('[data-campo="max_linhas"]', novo);
    await page.click('#cfSalvar');
    await dormir(600);
    const avisoSalvar = (await page.textContent('#aviso').catch(() => '')) || '';

    await page.reload({ waitUntil: 'domcontentloaded' });
    await page.waitForSelector('#btEntrar');
    await page.fill('#u', USUARIO); await page.fill('#s', SENHA); await page.fill('#t', TOKEN);
    await page.click('#btEntrar');
    await page.waitForSelector('#app.ativo[data-pronto="1"]', { timeout: 20000 });
    await page.evaluate(() => verConfigServidor());
    await dormir(400);
    const depois = await page.$eval('[data-campo="max_linhas"]', el => el.value);

    // A prova do disco, e nao so da tela: o config.json tem de trazer o
    // numero novo. Sem isto a tela poderia estar mostrando um cache seu.
    const noArquivo = JSON.parse(readFileSync(join(dir, 'config.json'), 'utf8')).max_linhas;

    relato.exercicioSalvar = { antes, novo, depoisDoReload: depois, avisoSalvar, noArquivo: String(noArquivo) };
    diz(`\nexercicio salvar/recarregar: antes=${antes} novo=${novo} depois-do-F5=${depois} config.json=${noArquivo}`);
    await ctx.close();
  }

  // -------------------------------------------------- exercicio: segredo
  {
    const ctx = await navegador.newContext({ viewport: { width: 1440, height: 900 } });
    const page = await ctx.newPage();
    await entrar(page, servidor.url);
    const cfg = await api(page, 'config');
    const bruto = JSON.stringify(cfg);
    await page.evaluate(() => verConfigServidor());
    await dormir(400);
    const domInteiro = await page.evaluate(() => document.documentElement.outerHTML);

    const achouNoJson = bruto.includes(SENHA_RELE) || bruto.includes(SENHA_CIFRA);
    const achouNoDom = domInteiro.includes(SENHA_RELE) || domInteiro.includes(SENHA_CIFRA);
    const campoEmailNoJson = cfg.alertas && cfg.alertas.email && cfg.alertas.email.senha;
    const campoCifraNoJson = cfg.cifra ? cfg.cifra.senha : null;

    relato.exercicioSegredo = {
      achouNoJson, achouNoDom, campoEmailNoJson, campoCifraNoJson,
      clusterNoJson: Object.prototype.hasOwnProperty.call(cfg, 'cluster'),
    };
    diz(`\nexercicio segredo: senha no JSON de config=${achouNoJson}  senha no DOM=${achouNoDom}`
      + `  alertas.email.senha="${campoEmailNoJson}"  cifra.senha="${campoCifraNoJson}"`
      + `  "cluster" aparece em config()=${relato.exercicioSegredo.clusterNoJson}`);
    await ctx.close();
  }

  writeFileSync(join(RAIZ, 'testes-web/.capturas-config-relato.json'), JSON.stringify(relato, null, 2));
  diz('\nrelato gravado em testes-web/.capturas-config-relato.json (nao versionado -- so para a resposta N.md)');
} finally {
  await navegador.close();
  await servidor.derrubar();
  try { rmSync(dir, { recursive: true, force: true }); } catch { /* tanto faz */ }
  diz('\nservidor derrubado pelo PID, diretorio temporario removido.');
}
