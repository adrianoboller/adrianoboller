/* Capturas do console para o dossie -- do login ate a replicacao.
 *
 * Sobe um phxsqld SO desta captura, na faixa 6700/6701, popula com dado que
 * nao deixa tela vazia, e fotografa o caminho que o dono pediu. Derruba pelo
 * PID -- nunca `pkill -f`, que mataria o servidor do vizinho.
 */
import { chromium } from '/opt/node22/lib/node_modules/playwright/index.mjs';
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { spawn, spawnSync } from 'node:child_process';
import { tmpdir } from 'node:os';
import { connect } from 'node:net';
import { join, resolve } from 'node:path';

const RAIZ = resolve(process.argv[2] || '.');
const SAIDA = resolve(process.argv[3] || './capturas');
const USUARIO = 'adm', SENHA = 'segredo1', TOKEN = 'dossie';

const PORTA_DADOS = 6700;
const PORTA_WEB = 6701;
const phxsqld = join(RAIZ, 'target/release/phxsqld');

const diz = (...a) => console.log(...a);
const dormir = ms => new Promise(r => setTimeout(r, ms));

/* --------------------------------------------------- o servidor da foto */

/* Papel `source` de proposito: com `isolado` a tela de Replicacao fotografa
 * a ausencia, e a secao do dossie trata do que existe. */
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

async function subir() {
  const dir = mkdtempSync(join(tmpdir(), 'phx-dossie-'));
  const base = join(dir, 'dados');
  const caminho = join(dir, 'config.json');
  writeFileSync(caminho, JSON.stringify({
    base, bind: `127.0.0.1:${PORTA_DADOS}`, token: TOKEN, max_linhas: 5000,
    web: { ligado: true, bind: `127.0.0.1:${PORTA_WEB}`, sessao_minutos: 60 },
    recursos: { durabilidade: 'sistema', cache_paginas: 512 },
    usuarios: [{
      id: 10, nome: 'Adriano Boller', login: USUARIO,
      senha_hash: hashDaSenha(SENHA), supervisor: true, ativo: true, bases: {},
    }],
    replicacao: { papel: 'source', id_servidor: 'matriz-01' },
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
  return {
    url: `http://127.0.0.1:${PORTA_WEB}/`,
    async derrubar() {
      matar();
      for (let i = 0; i < 60 && morreu === null; i++) await dormir(100);
      try { rmSync(dir, { recursive: true, force: true }); } catch { /* o /tmp limpa */ }
    },
  };
}

/* ------------------------------------------------------------ o cenario */

const CIDADES = ['Blumenau', 'Joinville', 'Curitiba', 'Florianópolis', 'Itajaí',
  'Brusque', 'Gaspar', 'Indaial', 'Timbó', 'Pomerode', 'Jaraguá do Sul', 'Chapecó'];
const UFS = ['SC', 'SC', 'PR', 'SC', 'SC', 'SC', 'SC', 'SC', 'SC', 'SC', 'SC', 'SC'];
const NOMES = ['Adriano Boller', 'Maria Souza', 'Carlos Lima', 'Helena Prado',
  'Rogério Antunes', 'Beatriz Falcão', 'Otávio Nunes', 'Vera Sampaio',
  'Nelson Bittencourt', 'Lúcia Meireles', 'Tiago Ferraz', 'Sônia Kremer',
  'Paulo Wagner', 'Íris Dantas', 'Gustavo Rebelo', 'Marta Küster'];

async function api(page, op, params = {}) {
  return await page.evaluate(([o, p]) => api(o, p), [op, params]);
}

async function popular(page, db) {
  await api(page, 'criar_database', { database: db }).catch(() => {});

  await api(page, 'criar_tabela', {
    database: db, tabela: 'clientes',
    colunas: [
      { nome: 'id', tipo: 'Int4', obrigatoria: true, caption: 'Código' },
      { nome: 'nome', tipo: 'Str(40)', obrigatoria: true, caption: 'Nome',
        dado_pessoal: 'pessoal' },
      { nome: 'cidade', tipo: 'Str(30)', caption: 'Cidade' },
      { nome: 'uf', tipo: 'Str(2)', caption: 'UF' },
      { nome: 'limite', tipo: 'Decimal(12,2)', caption: 'Limite', mascara: '@N-11.2' },
      { nome: 'cadastro', tipo: 'Date', caption: 'Cadastro' },
      { nome: 'ficha', tipo: 'Memo', caption: 'Ficha' },
    ],
    indices: [
      { nome: 'porId', colunas: ['id'], unico: true, primario: true },
      { nome: 'porNome', colunas: ['nome'], nocase: true },
      { nome: 'porCidade', colunas: ['cidade'], nocase: true },
    ],
  }).catch(e => diz('  clientes:', e));

  await api(page, 'criar_tabela', {
    database: db, tabela: 'pedidos',
    colunas: [
      { nome: 'numero', tipo: 'Int4', obrigatoria: true, caption: 'Número' },
      { nome: 'cliente', tipo: 'Int4', obrigatoria: true, caption: 'Cliente' },
      { nome: 'emissao', tipo: 'Date', caption: 'Emissão' },
      { nome: 'valor', tipo: 'Decimal(12,2)', caption: 'Valor' },
      { nome: 'situacao', tipo: 'Str(12)', caption: 'Situação' },
    ],
    indices: [
      { nome: 'porNumero', colunas: ['numero'], unico: true, primario: true },
      { nome: 'porCliente', colunas: ['cliente'] },
    ],
    chaves_estrangeiras: [{ nome: 'fkCliente', colunas: ['cliente'],
      tabela_ref: 'clientes', colunas_ref: ['id'] }],
  }).catch(e => diz('  pedidos:', e));

  await api(page, 'criar_tabela', {
    database: db, tabela: 'itens',
    colunas: [
      { nome: 'id', tipo: 'Int4', obrigatoria: true, caption: 'Item' },
      { nome: 'pedido', tipo: 'Int4', obrigatoria: true, caption: 'Pedido' },
      { nome: 'produto', tipo: 'Str(30)', caption: 'Produto' },
      { nome: 'qtd', tipo: 'Int4', caption: 'Qtd' },
      { nome: 'unitario', tipo: 'Decimal(12,2)', caption: 'Unitário' },
    ],
    indices: [
      { nome: 'porId', colunas: ['id'], unico: true, primario: true },
      { nome: 'porPedido', colunas: ['pedido'] },
    ],
    chaves_estrangeiras: [{ nome: 'fkPedido', colunas: ['pedido'],
      tabela_ref: 'pedidos', colunas_ref: ['numero'] }],
  }).catch(e => diz('  itens:', e));

  const clientes = [];
  for (let i = 1; i <= 240; i++) {
    const c = (i * 7) % CIDADES.length;
    clientes.push([i, `${NOMES[i % NOMES.length]} ${i}`, CIDADES[c], UFS[c],
      (500 + ((i * 137) % 24000)).toFixed(2),
      `202${3 + (i % 3)}-${String(1 + (i % 12)).padStart(2, '0')}-${String(1 + (i % 28)).padStart(2, '0')}`,
      i % 5 === 0 ? 'cliente antigo, contrato anual' : '']);
  }
  await api(page, 'inserir_lote', { database: db, tabela: 'clientes', linhas: clientes })
    .catch(async () => {
      for (const l of clientes) {
        await api(page, 'inserir', { database: db, tabela: 'clientes', valores: l }).catch(() => {});
      }
    });

  const pedidos = [];
  const SIT = ['aberto', 'faturado', 'entregue', 'cancelado'];
  for (let i = 1; i <= 420; i++) {
    pedidos.push([i, 1 + (i * 13) % 240,
      `202${4 + (i % 2)}-${String(1 + (i % 12)).padStart(2, '0')}-${String(1 + (i % 28)).padStart(2, '0')}`,
      (90 + ((i * 311) % 8000)).toFixed(2), SIT[i % 4]]);
  }
  await api(page, 'inserir_lote', { database: db, tabela: 'pedidos', linhas: pedidos }).catch(() => {});

  const itens = [];
  const PROD = ['Cabo HDMI', 'Fonte 12V', 'Placa mãe', 'Memória 16 GB',
    'SSD 1 TB', 'Teclado ABNT2', 'Monitor 24"', 'Roteador'];
  for (let i = 1; i <= 900; i++) {
    itens.push([i, 1 + (i * 7) % 420, PROD[i % PROD.length], 1 + (i % 9),
      (12 + ((i * 53) % 900)).toFixed(2)]);
  }
  await api(page, 'inserir_lote', { database: db, tabela: 'itens', linhas: itens }).catch(() => {});

  await api(page, 'criar_database', { database: 'Financeiro' }).catch(() => {});
  await api(page, 'criar_tabela', {
    database: 'Financeiro', tabela: 'titulos',
    colunas: [
      { nome: 'id', tipo: 'Int4', obrigatoria: true },
      { nome: 'sacado', tipo: 'Str(40)' },
      { nome: 'vencimento', tipo: 'Date' },
      { nome: 'valor', tipo: 'Decimal(12,2)' },
    ],
    indices: [{ nome: 'porId', colunas: ['id'], unico: true, primario: true }],
  }).catch(() => {});
  const tit = [];
  for (let i = 1; i <= 120; i++) {
    tit.push([i, `${NOMES[i % NOMES.length]}`,
      `2026-${String(1 + (i % 12)).padStart(2, '0')}-${String(1 + (i % 28)).padStart(2, '0')}`,
      (300 + ((i * 91) % 5000)).toFixed(2)]);
  }
  await api(page, 'inserir_lote', { database: 'Financeiro', tabela: 'titulos', linhas: tit }).catch(() => {});
}

/** Movimento suficiente para o painel e a telemetria terem o que mostrar. */
async function movimentar(page, db) {
  for (let i = 0; i < 60; i++) {
    await api(page, 'varrer', { database: db, tabela: 'clientes', max: 50 }).catch(() => {});
    await api(page, 'ler', { database: db, tabela: 'pedidos', rowid: 1 + (i * 3) }).catch(() => {});
    await api(page, 'esquema', { database: db, tabela: 'itens' }).catch(() => {});
    if (i % 7 === 0) await api(page, 'tabelas', { database: db }).catch(() => {});
    if (i % 11 === 0) await api(page, 'varrer', { database: 'Financeiro', tabela: 'titulos', max: 120 }).catch(() => {});
  }
}

/* ------------------------------------------------------------ a captura */

async function foto(page, nome, opc = {}) {
  mkdirSync(SAIDA, { recursive: true });
  // O ponteiro longe de tudo: o realce de linha por `:hover` entra na foto e
  // parece defeito de pintura para quem olha depois.
  await page.mouse.move(4, 4);
  await dormir(opc.espera || 500);
  const arq = join(SAIDA, `${nome}.png`);
  await page.screenshot({ path: arq, fullPage: !!opc.inteira });
  diz(`  ✓ ${nome}.png`);
  return arq;
}

async function entrar(page, url) {
  await page.goto(url, { waitUntil: 'domcontentloaded' });
  await page.waitForSelector('#btEntrar');
  await page.waitForFunction(() => typeof est === 'object' && est.demo === false,
    { timeout: 20000 });
  await page.fill('#u', USUARIO);
  await page.fill('#s', SENHA);
  await page.fill('#t', TOKEN);
}

async function abrirApp(page) {
  await page.click('#btEntrar');
  await page.waitForSelector('#app.ativo', { timeout: 25000 });
  await page.waitForSelector('#arvore .no', { timeout: 25000 });
  await dormir(700);
}

async function rodada(navegador, servidor, tema, quais, primeira) {
  const ctx = await navegador.newContext({
    viewport: { width: 1500, height: 900 }, deviceScaleFactor: 1,
  });
  await ctx.addInitScript(t => {
    try { localStorage.setItem('phxsql-tema', t); } catch { /* privado */ }
  }, tema);
  const page = await ctx.newPage();
  const erros = [];
  page.on('pageerror', e => erros.push(e.message || String(e)));

  const db = 'Comercial';
  const q = n => quais.includes(n);

  await entrar(page, servidor.url);
  if (q('login')) await foto(page, `login-${tema}`, { espera: 1400 });
  await abrirApp(page);

  if (primeira) {
    diz('  populando…');
    await popular(page, db);
    await movimentar(page, db);
    await page.reload({ waitUntil: 'domcontentloaded' });
    await page.waitForSelector('#btEntrar');
    await page.fill('#u', USUARIO); await page.fill('#s', SENHA); await page.fill('#t', TOKEN);
    await abrirApp(page);
  }

  // --- painel
  if (q('painel')) {
    await page.evaluate(() => irPara('painel'));
    await foto(page, `painel-${tema}`, { espera: 2500 });
  }

  // --- tabelas (gestao)
  if (q('tabelas')) {
    await page.evaluate(d => { est.db = d; }, db);
    await page.evaluate(() => gerirTabelasAtual());
    await foto(page, `tabelas-${tema}`, { espera: 1400 });
  }

  // --- grade (conteudo da tabela)
  if (q('grade')) {
    await page.evaluate(([d, t]) => abrirTabela(d, t), [db, 'clientes']);
    await dormir(1800);
    await page.evaluate(() => {
      const a = document.querySelector('[data-aba="conteudo"]');
      if (a) a.click();
    });
    await foto(page, `grade-${tema}`, { espera: 2200 });
  }

  // --- query: a tabela vai para a RAM antes, senao a tela so explica
  if (q('query')) {
    await api(page, 'memoria_carregar', { database: db, tabela: 'clientes' })
      .catch(() => {});
    
    await page.evaluate(() => abrirConsulta());
    await dormir(900);
    await page.evaluate(([d, t]) => {
      const p = (s, v) => {
        const e = document.querySelector(s);
        if (e) { e.value = v; e.dispatchEvent(new Event('input', { bubbles: true })); }
      };
      p('#cDb', d); p('#cTab', t); p('#cCol', 'uf'); p('#cVal', 'SC');
    }, [db, 'clientes']);
    const bt = await page.$('#btConsultar, button:has-text("Consultar")');
    if (bt) await bt.click().catch(() => {});
    await foto(page, `query-${tema}`, { espera: 1800 });
  }

  // --- diagrama ER: reorganizado, e com a lateral recolhida para caber
  if (q('diagrama')) {
    await page.evaluate(d => telaDiagramaER(d), db);
    await dormir(2000);
    const re = await page.$('button:has-text("Reorganizar")');
    if (re) { await re.click().catch(() => {}); await dormir(900); }
    await foto(page, `diagrama-${tema}`, { espera: 1200 });
  }

  // --- telemetria: as bolhas ficam ABAIXO da dobra; a foto desce ate elas
  if (q('telemetria')) {
    await page.evaluate(() => telaTelemetria());
    await dormir(1500);
    await api(page, 'telemetria_ligar', {}).catch(() => {});
    await page.evaluate(async d => {
      for (let i = 0; i < 40; i++) {
        await api('varrer', { database: d, tabela: 'clientes', max: 60 }).catch(() => {});
        await api('varrer', { database: d, tabela: 'itens', max: 200 }).catch(() => {});
        await api('ler', { database: d, tabela: 'pedidos', rowid: 1 + i }).catch(() => {});
        await api('esquema', { database: d, tabela: 'pedidos' }).catch(() => {});
      }
    }, db);
    await dormir(2500);
    await page.evaluate(() => {
      const p = document.querySelector('#painel');
      if (p) p.scrollTop = 430;
    });
    await foto(page, `telemetria-${tema}`, { espera: 1500 });
  }

  // --- profiler
  if (q('profiler')) {
    await page.evaluate(() => verProfiler());
    await dormir(1200);
    const lig = await page.$('#pfLigar, [data-acao="profiler-ligar"]');
    if (lig) await lig.click().catch(() => {});
    await dormir(400);
    await page.evaluate(async d => {
      for (let i = 0; i < 25; i++) {
        await api('varrer', { database: d, tabela: 'clientes', max: 40 }).catch(() => {});
        await api('ler', { database: d, tabela: 'clientes', rowid: 1 + i }).catch(() => {});
        await api('tabelas', { database: d }).catch(() => {});
      }
    }, db);
    await foto(page, `profiler-${tema}`, { espera: 2200 });
  }

  // --- replicacao
  if (q('replicacao')) {
    await page.evaluate(() => verReplicacao());
    await foto(page, `replicacao-${tema}`, { espera: 1800 });
  }

  // --- multitela: quatro telas lado a lado
  if (q('multitela')) {
    await page.setViewportSize({ width: 2800, height: 1050 });
    await dormir(700);
    await page.evaluate(async d => {
      const W = PhxTelas._W;
      PhxTelas.dividir(4);
      const r = W.regioes;
      await PhxTelas.abrir('diagrama', { db: d }, { regiao: r[0] });
      await PhxTelas.abrir('telemetria', {}, { regiao: r[1], nova: true });
      await PhxTelas.abrir('profiler', {}, { regiao: r[2], nova: true });
      await PhxTelas.abrir('query', {}, { regiao: r[3], nova: true });
    }, db).catch(e => diz('  multitela:', e.message));
    await foto(page, `multitela-${tema}`, { espera: 4000 });
  }

  if (erros.length) diz(`  ⚠ pageerror: ${erros.slice(0, 4).join(' | ')}`);
  await ctx.close();
}

/* ------------------------------------------------------------------ main */

const servidor = await subir();
diz(`servidor no ar: ${servidor.url}`);
const navegador = await chromium.launch();
try {
  const todas = ['login', 'painel', 'tabelas', 'grade', 'query', 'diagrama',
    'telemetria', 'profiler', 'replicacao', 'multitela'];
  diz('\n── tema escuro ──');
  await rodada(navegador, servidor, 'escuro', todas, true);
  diz('\n── tema claro ──');
  await rodada(navegador, servidor, 'claro', todas, false);
} finally {
  await navegador.close();
  await servidor.derrubar();
  diz('\nservidor derrubado pelo PID.');
}
