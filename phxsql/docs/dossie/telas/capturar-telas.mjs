/* Telas do console web em SVG -- do servidor de verdade, nos dois temas.
 *
 * O inventario NAO e digitado: sai do proprio `ui/index.html` em tempo de
 * execucao -- a constante `MENUS` (a barra de menus) e a `ABAS` (as abas da
 * tabela), lidas de dentro da pagina servida pelo `http.rs`. Cada item e
 * aberto CLICANDO no menu, como uma pessoa faria.
 *
 * Uso: node docs/dossie/telas/capturar-telas.mjs . <pasta-de-saida>
 * Depois: os PNG dos SVG -escuro sobem ao deposito da pagina das telas, e
 * `docs/dossie/telas/ativos.json` guarda a URL de cada um (pagina-das-telas.py).
 * Sobe um phxsqld proprio em 6750/6751 e derruba so o PID dele.
 */
import { chromium } from '/opt/node22/lib/node_modules/playwright/index.mjs';
import { mkdirSync, mkdtempSync, rmSync, writeFileSync, readFileSync, statSync } from 'node:fs';
import { spawn, spawnSync } from 'node:child_process';
import { connect } from 'node:net';
import { join, resolve } from 'node:path';

const RAIZ = resolve(process.argv[2] || '.');
const SAIDA = resolve(process.argv[3] || './telas-svg');
const USUARIO = 'adm', SENHA = 'segredo1', TOKEN = 'telas';
const PORTA_DADOS = 6750, PORTA_WEB = 6751;
const phxsqld = join(RAIZ, 'target/release/phxsqld');
// O commit e o da ARVORE QUE COMPILOU o binario, nao o HEAD da hora da
// foto: numa rodada com frentes paralelas o HEAD andou duas vezes durante a
// captura. Passe COMMIT_DO_BINARIO quando o HEAD puder ter andado.
const COMMIT = process.env.COMMIT_DO_BINARIO || spawnSync('git', ['-C', RAIZ, 'rev-parse', '--short', 'HEAD'], { encoding: 'utf8' }).stdout.trim();
const W = 1500, H = 900;

const diz = (...a) => console.log(...a);
const dormir = ms => new Promise(r => setTimeout(r, ms));
const esc = s => String(s).replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');
const slug = s => s.normalize('NFD').replace(/[̀-ͯ]/g, '').toLowerCase()
  .replace(/[^a-z0-9]+/g, '-').replace(/^-|-$/g, '');

/* Itens que NAO sao tela: mudam o layout, o tema ou encerram a sessao. O
 * motivo vai para o LEIA-ME -- dispensa registrada, nao silenciosa. */
const NAO_E_TELA = {
  'tela.sair': 'encerra a sessão (volta ao login, já fotografado)',
  'tela.mi_tema': 'alterna o tema (as duas versões já estão fotografadas)',
  'tela.mi_atualizar': 'recarrega a vista atual',
  'tela.mi_nova_aba': 'ação de layout (abre aba em branco)',
  'tela.mi_fechar_aba': 'ação de layout',
  'tela.mi_uma_regiao': 'ação de layout',
  'tela.mi_duas_regioes': 'ação de layout',
  'tela.mi_tres_regioes': 'ação de layout',
  'tela.mi_soltar': 'ação de layout (janela flutuante da tela atual)',
  'tela.mi_alinhar': 'ação de layout',
};

/* ------------------------------------------------------------- servidor */

function hashDaSenha(senha) {
  const r = spawnSync(phxsqld, ['--senha'], { input: senha, encoding: 'utf8' });
  const m = /"senha_hash": "([^"]+)"/.exec(r.stdout || '');
  if (!m) throw new Error(`phxsqld --senha nao devolveu o hash: ${r.stdout}${r.stderr}`);
  return m[1];
}
async function esperarPorta(porta, prazoMs = 20000) {
  const fim = Date.now() + prazoMs;
  while (Date.now() < fim) {
    const ok = await new Promise(r => {
      const s = connect({ host: '127.0.0.1', port: porta }, () => { s.destroy(); r(true); });
      s.on('error', () => r(false));
      s.setTimeout(500, () => { s.destroy(); r(false); });
    });
    if (ok) return true;
    await dormir(150);
  }
  return false;
}
async function subir() {
  const dir = mkdtempSync(join(resolve(SAIDA, '..'), 'phx-telas-'));
  const caminho = join(dir, 'config.json');
  writeFileSync(caminho, JSON.stringify({
    base: join(dir, 'dados'), bind: `127.0.0.1:${PORTA_DADOS}`, token: TOKEN, max_linhas: 5000,
    web: { ligado: true, bind: `127.0.0.1:${PORTA_WEB}`, sessao_minutos: 60, tls: false /* 770: http em claro e sessao de 60 min POR ESCRITO -- a bateria nao fala TLS */ },
    recursos: { durabilidade: 'sistema', cache_paginas: 512 },
    usuarios: [{ id: 10, nome: 'Adriano Boller', login: USUARIO,
      senha_hash: hashDaSenha(SENHA), supervisor: true, ativo: true, bases: {} }],
    replicacao: { papel: 'source', id_servidor: 'matriz-01' },
    // Servidor de captura so em 127.0.0.1: sem isto a porta HTTP em texto
    // puro devolve 403 (cifra_fio.exigir nasce ligado).
    cifra_fio: { exigir: false },
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
    throw new Error(`portas nao abriram (saida=${morreu}):\n${saida.join('')}`);
  }
  return {
    url: `http://127.0.0.1:${PORTA_WEB}/`, pid: proc.pid,
    async derrubar() {
      matar();
      for (let i = 0; i < 60 && morreu === null; i++) await dormir(100);
      rmSync(dir, { recursive: true, force: true });
    },
  };
}

/* -------------------------------------------------------------- cenario */

const CIDADES = ['Blumenau', 'Joinville', 'Curitiba', 'Florianópolis', 'Itajaí', 'Brusque'];
const NOMES = ['Adriano Boller', 'Maria Souza', 'Carlos Lima', 'Helena Prado', 'Rogério Antunes', 'Beatriz Falcão'];
const api = (page, op, p = {}) => page.evaluate(([o, q]) => api(o, q), [op, p]);

async function popular(page, db) {
  await api(page, 'criar_database', { database: db }).catch(() => {});
  await api(page, 'criar_tabela', { database: db, tabela: 'clientes',
    colunas: [
      { nome: 'id', tipo: 'Int4', obrigatoria: true, caption: 'Código' },
      { nome: 'nome', tipo: 'Str(40)', obrigatoria: true, caption: 'Nome', dado_pessoal: 'pessoal' },
      { nome: 'cidade', tipo: 'Str(30)', caption: 'Cidade' },
      { nome: 'uf', tipo: 'Str(2)', caption: 'UF' },
      { nome: 'limite', tipo: 'Decimal(12,2)', caption: 'Limite' },
      { nome: 'cadastro', tipo: 'Date', caption: 'Cadastro' },
    ],
    indices: [
      { nome: 'porId', colunas: ['id'], unico: true, primario: true },
      { nome: 'porNome', colunas: ['nome'], nocase: true },
    ] }).catch(e => diz('  clientes:', e));
  await api(page, 'criar_tabela', { database: db, tabela: 'pedidos',
    colunas: [
      { nome: 'numero', tipo: 'Int4', obrigatoria: true, caption: 'Número' },
      { nome: 'cliente', tipo: 'Int4', obrigatoria: true, caption: 'Cliente' },
      { nome: 'valor', tipo: 'Decimal(12,2)', caption: 'Valor' },
      { nome: 'situacao', tipo: 'Str(12)', caption: 'Situação' },
    ],
    indices: [
      { nome: 'porNumero', colunas: ['numero'], unico: true, primario: true },
      { nome: 'porCliente', colunas: ['cliente'] },
    ],
    chaves_estrangeiras: [{ nome: 'fkCliente', colunas: ['cliente'], tabela_ref: 'clientes', colunas_ref: ['id'] }],
  }).catch(e => diz('  pedidos:', e));
  const cli = [];
  for (let i = 1; i <= 120; i++) {
    const c = (i * 7) % CIDADES.length;
    cli.push([i, `${NOMES[i % NOMES.length]} ${i}`, CIDADES[c], c === 2 ? 'PR' : 'SC',
      (500 + ((i * 137) % 24000)).toFixed(2), `2025-${String(1 + (i % 12)).padStart(2, '0')}-${String(1 + (i % 28)).padStart(2, '0')}`]);
  }
  await api(page, 'inserir_lote', { database: db, tabela: 'clientes', linhas: cli }).catch(e => diz('  lote cli:', e));
  const SIT = ['aberto', 'faturado', 'entregue', 'cancelado'];
  const ped = [];
  for (let i = 1; i <= 200; i++) ped.push([i, 1 + (i * 13) % 120, (90 + ((i * 311) % 8000)).toFixed(2), SIT[i % 4]]);
  await api(page, 'inserir_lote', { database: db, tabela: 'pedidos', linhas: ped }).catch(e => diz('  lote ped:', e));
  for (let i = 0; i < 20; i++) {
    await api(page, 'varrer', { database: db, tabela: 'clientes', max: 50 }).catch(() => {});
    await api(page, 'ler', { database: db, tabela: 'pedidos', rowid: 1 + i }).catch(() => {});
  }
}

/* -------------------------------------------------------------- captura */

async function entrar(page, url) {
  await page.goto(url, { waitUntil: 'domcontentloaded' });
  await page.waitForSelector('#btEntrar');
  await page.waitForFunction(() => typeof est === 'object' && est.demo === false, { timeout: 20000 });
  await page.fill('#u', USUARIO); await page.fill('#s', SENHA); await page.fill('#t', TOKEN);
}
async function abrirApp(page) {
  await page.click('#btEntrar');
  await page.waitForSelector('#app.ativo', { timeout: 25000 });
  await page.waitForSelector('#arvore .no', { timeout: 25000 });
  await dormir(600);
}

/** Recado de erro visivel AGORA: o `#aviso.mal` e o `.aviso.mal` do painel. */
async function recadoDeErro(page) {
  return await page.evaluate(() => {
    // `offsetParent` e null para `position:fixed` -- e o `#aviso` e fixo: o
    // primeiro detector aprovava tudo por isso.
    const vis = e => e && !e.hidden && e.getClientRects().length > 0
      && getComputedStyle(e).visibility !== 'hidden' && getComputedStyle(e).display !== 'none';
    const out = [];
    const a = document.querySelector('#aviso');
    if (a && a.classList.contains('mal') && vis(a)) out.push(a.textContent.trim());
    document.querySelectorAll('.aviso.mal, .recado.mal').forEach(e => {
      if (e.id !== 'aviso' && vis(e)) out.push(e.textContent.trim());
    });
    return [...new Set(out)].map(s => s.slice(0, 160));
  });
}

function pngTamanho(buf) { return { w: buf.readUInt32BE(16), h: buf.readUInt32BE(20) }; }

function gravarSvg(nomeArq, png, titulo) {
  const { w, h } = pngTamanho(png);
  const svg = `<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="${w}" height="${h}" viewBox="0 0 ${w} ${h}">
  <title>${esc(titulo)}</title>
  <desc>Captura fiel do console web do PhxSql (servidor real, commit ${COMMIT}). Contêiner vetorial com a imagem da tela; nada redesenhado à mão.</desc>
  <image width="${w}" height="${h}" href="data:image/png;base64,${png.toString('base64')}" xlink:href="data:image/png;base64,${png.toString('base64')}"/>
</svg>
`;
  // Um so href basta para os navegadores atuais; o xlink:href duplicaria o
  // peso. Fica so o `href`.
  const enxuto = svg.replace(/ xlink:href="[^"]*"/, '');
  writeFileSync(join(SAIDA, nomeArq), enxuto);
  return { w, h };
}

const resultado = []; // {ordem, menu, item, chave, tema, arq, w, h, erros:[], obs}

async function rodada(nav, srv, tema, inventarioPronto) {
  const ctx = await nav.newContext({ viewport: { width: W, height: H }, deviceScaleFactor: 1 });
  await ctx.addInitScript(t => { try { localStorage.setItem('phxsql-tema', t); } catch {} }, tema);
  const page = await ctx.newPage();
  let errosPg = [];
  let dialogo = null;
  page.on('pageerror', e => errosPg.push(e.message || String(e)));
  page.on('dialog', d => { dialogo = `${d.type()}: ${d.message().split('\n')[0].slice(0, 120)}`; d.dismiss().catch(() => {}); });

  const db = 'Comercial';
  const foto = async (base, meta, espera = 1500) => {
    await page.mouse.move(4, 4);
    await dormir(espera);
    const png = await page.screenshot();
    const arq = `${base}-${tema}.svg`;
    const titulo = `PhxSql — ${meta.menu ? meta.menu + ' › ' : ''}${meta.item} — tema ${tema} — commit ${COMMIT}`;
    const { w, h } = gravarSvg(arq, png, titulo);
    const erros = [...(await recadoDeErro(page)), ...errosPg.map(e => `pageerror: ${e.slice(0, 160)}`)];
    resultado.push({ ...meta, tema, arq, w, h, erros, png });
    diz(`  ${erros.length ? '!' : '✓'} ${arq}${erros.length ? '  ' + erros.join(' | ') : ''}`);
  };

  // 1. login
  await entrar(page, srv.url);
  errosPg = [];
  await foto('00-login', { ordem: 0, menu: '', item: 'Login', chave: 'login' }, 1200);
  await abrirApp(page);
  if (!inventarioPronto) { diz('  populando…'); await popular(page, db); }

  // inventario: da pagina, nao de memoria
  const inv = await page.evaluate(() => {
    const L = [];
    MENUS.forEach(([nome, , chaveMenu, itens], m) => itens.forEach((it, i) => {
      if (it === 'sep') return;
      L.push({ m, i, menu: txt(chaveMenu, nome), item: txt(it.txt, it.rot), chave: it.txt || it.rot,
        falta: it.falta || '', comTabela: !!it.quando });
    }));
    return { itens: L, abas: ABAS.map(([k, r, c]) => ({ k, item: txt(c, r) })) };
  });

  const reiniciar = async () => {
    await entrar(page, srv.url);
    await abrirApp(page);
    await page.evaluate(([d, t]) => abrirTabela(d, t), [db, 'clientes']);
    await dormir(900);
    errosPg = []; dialogo = null;
    await page.evaluate(() => { const a = document.querySelector('#aviso'); if (a) a.hidden = true; });
  };

  // 2. abas da tabela
  let ordem = 1;
  for (const a of inv.abas) {
    await reiniciar();
    await page.click(`#abas [data-aba="${a.k}"]`).catch(() => {});
    await foto(`${String(ordem).padStart(2, '0')}-tabela-${slug(a.item)}`,
      { ordem, menu: 'Tabela (abas)', item: a.item, chave: `aba.${a.k}` }, 1800);
    ordem++;
  }

  // 3. cada item do menu, pelo clique
  const vistos = new Set();
  for (const it of inv.itens) {
    const nomeBase = `${String(ordem).padStart(2, '0')}-${slug(it.menu)}-${slug(it.item)}`;
    const meta = { ordem, menu: it.menu, item: it.item, chave: it.chave };
    ordem++;
    if (it.falta) { resultado.push({ ...meta, tema, arq: '', obs: 'item desligado no código (ainda não existe): ' + it.falta.slice(0, 90) + '…' }); continue; }
    if (NAO_E_TELA[it.chave]) { resultado.push({ ...meta, tema, arq: '', obs: 'não é tela: ' + NAO_E_TELA[it.chave] }); continue; }
    const quatro = it.chave === 'tela.mi_quatro_regioes';
    if (quatro) await page.setViewportSize({ width: 3000, height: 1050 });
    await reiniciar();
    await page.click(`.menubar .titulo[data-m="${it.m}"]`);
    await dormir(200);
    const bt = page.locator(`.menubar .item[data-m="${it.m}"][data-i="${it.i}"]`);
    if (await bt.isDisabled()) {
      resultado.push({ ...meta, tema, arq: '', obs: 'item cinza no estado do cenário' });
      if (quatro) await page.setViewportSize({ width: W, height: H });
      continue;
    }
    await bt.click();
    if (quatro) {
      // As quatro regioes so mostram algo com uma tela em cada.
      await dormir(700);
      await page.evaluate(async d => {
        const r = PhxTelas._W.regioes;
        await PhxTelas.abrir('diagrama', { db: d }, { regiao: r[0] });
        await PhxTelas.abrir('telemetria', {}, { regiao: r[1], nova: true });
        await PhxTelas.abrir('profiler', {}, { regiao: r[2], nova: true });
        await PhxTelas.abrir('query', {}, { regiao: r[3], nova: true });
      }, db).catch(e => errosPg.push('multitela: ' + e.message));
    }
    await dormir(400);
    if (dialogo) {
      resultado.push({ ...meta, tema, arq: '', obs: `abre diálogo NATIVO do navegador (${dialogo}); não se fotografa, e foi cancelado` });
      continue;
    }
    const lenta = /telemetria|profiler|diagrama|painel/i.test(it.chave);
    await foto(nomeBase, meta, quatro ? 4000 : lenta ? 2600 : 1600);
    if (quatro) await page.setViewportSize({ width: W, height: H });
    vistos.add(it.chave);
  }
  await ctx.close();
}

/* --------------------------------------------- miniaturas e indice.svg */

async function miniaturas(nav) {
  const ctx = await nav.newContext({ viewport: { width: 300, height: 180 }, deviceScaleFactor: 1 });
  const page = await ctx.newPage();
  for (const r of resultado.filter(x => x.png)) {
    const tw = 300, th = Math.round(300 * r.h / r.w);
    await page.setViewportSize({ width: tw, height: th });
    await page.setContent(`<body style="margin:0"><img style="width:${tw}px;height:${th}px;display:block" src="data:image/png;base64,${r.png.toString('base64')}"></body>`);
    await page.waitForFunction(() => document.images[0].complete);
    r.mini = await page.screenshot({ type: 'jpeg', quality: 70 });
    r.mw = tw; r.mh = th;
  }
  await ctx.close();
}

function indiceSvg() {
  const fot = resultado.filter(x => x.png);
  // Uma linha por tela, os dois temas lado a lado: comparar e o uso.
  const telas = [...new Map(fot.map(r => [r.arq.replace(/-(claro|escuro)\.svg$/, ''), r])).keys()];
  const COLS = 4, CW = 300, GAP = 24, ROT = 38, CH = 180;
  const celW = CW * 2 + 8, celH = CH + ROT;
  const larg = COLS * celW + (COLS + 1) * GAP;
  const linhas = Math.ceil(telas.length / COLS);
  const topo = 96;
  const alt = topo + linhas * (celH + GAP) + GAP;
  let s = `<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" width="${larg}" height="${alt}" viewBox="0 0 ${larg} ${alt}">
  <title>PhxSql — telas do console web — commit ${COMMIT}</title>
  <rect width="100%" height="100%" fill="#010418"/>
  <text x="${GAP}" y="44" fill="#ffffff" font-family="'Exo 2', sans-serif" font-size="28" font-weight="700">PhxSql — telas do console web</text>
  <text x="${GAP}" y="74" fill="#9aa4c7" font-family="'Exo 2', sans-serif" font-size="15">${telas.length} telas × 2 temas (escuro | claro) · commit ${COMMIT} · capturas do servidor real · Built to store. Engineered to scale.</text>
`;
  telas.forEach((base, k) => {
    const x = GAP + (k % COLS) * (celW + GAP), y = topo + Math.floor(k / COLS) * (celH + GAP);
    const par = ['escuro', 'claro'].map(t => fot.find(r => r.arq === `${base}-${t}.svg`));
    const ref = par[0] || par[1];
    const erro = par.some(r => r && r.erros.length);
    s += `  <g>\n    <text x="${x}" y="${y + 16}" fill="${erro ? '#ff5c5c' : '#e6e9f5'}" font-family="'Exo 2', sans-serif" font-size="14" font-weight="600">${esc((ref.menu ? ref.menu + ' › ' : '') + ref.item)}${erro ? ' — COM ERRO' : ''}</text>\n`;
    par.forEach((r, j) => {
      const ix = x + j * (CW + 8), iy = y + ROT - 14;
      if (!r) { s += `    <rect x="${ix}" y="${iy}" width="${CW}" height="${CH}" fill="none" stroke="#444"/>\n`; return; }
      s += `    <a href="${r.arq}"><image x="${ix}" y="${iy}" width="${CW}" height="${r.mh}" href="data:image/jpeg;base64,${r.mini.toString('base64')}"/>`
        + `<rect x="${ix}" y="${iy}" width="${CW}" height="${r.mh}" fill="none" stroke="${r.erros.length ? '#ff5c5c' : '#2c3566'}"/></a>\n`;
    });
    s += `  </g>\n`;
  });
  return s + '</svg>\n';
}

function leiaMe() {
  const kb = f => (statSync(join(SAIDA, f)).size / 1024).toFixed(0) + ' KiB';
  const fot = resultado.filter(x => x.arq);
  const com = fot.filter(x => x.erros.length);
  const nao = resultado.filter(x => !x.arq);
  const telas = new Set(fot.map(r => r.arq.replace(/-(claro|escuro)\.svg$/, '')));
  let t = `# Telas do console web do PhxSql em SVG

Commit \`${COMMIT}\` (a árvore que compilou o \`phxsqld\`; a \`ui/\` e o \`http.rs\` não mudaram até o HEAD da hora da captura) · gerado em ${new Date().toISOString()} · viewport ${W}×${H} (a multitela em 3000×1050: cada região pede 660 px, e 4 regiões não cabem em 2800 com a lateral aberta).

**${telas.size} telas × 2 temas = ${fot.length} SVGs**, mais o \`indice.svg\` (grade de miniaturas).

Cada SVG é um **contêiner vetorial com a captura fiel** da tela (PNG em base64 dentro de \`<image>\`),
com \`<title>\` dizendo tela, tema e commit. Nada foi redesenhado à mão.

## De onde sai o inventário

Do próprio código, em tempo de execução: as constantes \`MENUS\` (barra de menus) e \`ABAS\` (abas da
tabela) do \`crates/phxsql-server/ui/index.html\`, embutido pelo \`http.rs\`. Cada item foi aberto
**clicando no menu**, com a tabela \`Comercial/clientes\` aberta (para os itens que exigem tabela).
O servidor de captura escuta só em 127.0.0.1 com \`cifra_fio.exigir: false\` (sem isso a porta HTTP
em texto puro responde 403 — o padrão exige cifra). O cenário: database \`Comercial\` com \`clientes\` (120 linhas) e \`pedidos\` (200, FK para clientes).

## Telas com erro (recado vermelho ou pageerror)

`;
  t += com.length
    ? com.map(r => `- **${r.menu ? r.menu + ' › ' : ''}${r.item}** (${r.tema}, \`${r.arq}\`): ${r.erros.join(' | ')}`).join('\n')
    : '- nenhuma';
  t += `\n\n## Itens do menu que não viraram SVG, e por quê\n\n`;
  const porChave = new Map();
  for (const r of nao) if (!porChave.has(r.chave)) porChave.set(r.chave, r);
  t += [...porChave.values()].map(r => `- ${r.menu} › ${r.item} — ${r.obs}`).join('\n');
  t += `\n\n## Lista\n\n| # | Tela | Tema | Arquivo | Tamanho | Erro |\n|---|---|---|---|---|---|\n`;
  t += fot.map(r => `| ${r.ordem} | ${(r.menu ? r.menu + ' › ' : '') + r.item} | ${r.tema} | \`${r.arq}\` | ${kb(r.arq)} | ${r.erros.length ? 'sim' : '—'} |`).join('\n');
  t += `\n| — | Índice (miniaturas) | ambos | \`indice.svg\` | ${kb('indice.svg')} | — |\n`;
  t += `
## Como refazer

\`\`\`bash
cd /home/user/adrianoboller/phxsql
export CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=2
cargo build --release -p phxsql-server
export COMMIT_DO_BINARIO=$(git rev-parse --short HEAD)   # o commit que compilou
node <rascunho>/telas-svg-gerar.mjs /home/user/adrianoboller/phxsql <rascunho>/telas-svg
(cd <rascunho>/telas-svg && zip -q -r ../telas-svg.zip . && mv ../telas-svg.zip .)
\`\`\`

O script sobe um \`phxsqld\` próprio em 127.0.0.1:6750/6751, popula, fotografa os dois temas e derruba
**só o PID dele**. Precisa do Playwright em \`/opt/node22/lib/node_modules/playwright\`.
`;
  return t;
}

/* ----------------------------------------------------------------- main */

rmSync(SAIDA, { recursive: true, force: true });
mkdirSync(SAIDA, { recursive: true });
const srv = await subir();
const nav = await chromium.launch();
try {
  diz('\n── tema escuro ──');
  await rodada(nav, srv, 'escuro', false);
  diz('\n── tema claro ──');
  await rodada(nav, srv, 'claro', true);
  await miniaturas(nav);
  writeFileSync(join(SAIDA, 'indice.svg'), indiceSvg());
  writeFileSync(join(SAIDA, 'LEIA-ME.md'), leiaMe());
  writeFileSync(join(SAIDA, '..', 'telas-svg-resultado.json'), JSON.stringify(
    resultado.map(({ png, mini, ...r }) => r), null, 1));
} finally {
  await nav.close();
  await srv.derrubar();
  diz(`\nservidor ${srv.pid} derrubado pelo PID.`);
}
