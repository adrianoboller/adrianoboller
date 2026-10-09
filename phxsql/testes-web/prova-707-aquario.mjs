/* Prova pelo navegador das fatias A10 e A11 do pedido 707 -- o aquario ligado
 * ao servidor REAL (papel E, 09/10/2026). Roda nos DOIS temas.
 *
 *   node testes-web/prova-707-aquario.mjs [caminho/do/phxsqld]   (da pasta phxsql/)
 *
 * Carga de verdade: um usuario que nao e a TV (`joana.carga`, entrando de
 * 127.0.0.3 pela porta de dados) agrupa uma tabela de 300 mil linhas em quatro
 * conexoes, e a trava de dados faz as outras esperarem -- daí saem bolhas
 * de tempos e cores diferentes, sem retrato inventado.
 *
 * O que cada passo MEDE:
 *   cor      cada bolha do DOM tem a cor (data-cor e o `stroke` computado) da
 *            tarefa no ULTIMO retrato que a tela recebeu;
 *   faixas   o nome das faixas e desenhado DEPOIS das bolhas (por cima);
 *   dado     o nome da tabela sai como foi gravado («Vendas_Blumenau»), sem
 *            caixa alta imposta pelo CSS global;
 *   inserir  um `inserir` real sobe em exatamente 1 a barra «inclusão» do dia;
 *   oculta   aba escondida faz 0 pedidos do aquario em 6 s, e volta a pedir
 *            ao reaparecer;
 *   lgpd     com o login so de `monitorar`, nem o login nem o IP de quem
 *            gera a carga aparecem no DOM -- e o retrato do administrador os
 *            TEM, senao a prova passaria por falta de dado;
 *   alca     a alca da telemetria abre o aquario, estica e guarda a altura,
 *            e fechada volta a 0 pedidos;
 *   console  nenhum erro de console nem de pagina;
 *   contraste  rotulo das faixas, legenda e linha do log >= 4,5:1 nos dois temas.
 *
 * Sai com codigo 1 se qualquer medida reprovar. Capturas em target/. */
import { chromium } from '/opt/node22/lib/node_modules/playwright/index.mjs';
import { spawnSync } from 'node:child_process';
import { connect } from 'node:net';
import { mkdirSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { subir, USUARIO, SENHA, TOKEN } from './servidor.mjs';
import { entrar } from './apoio.mjs';

const PHXSQLD = resolve(process.argv[2] || 'target/debug/phxsqld');
const SAIDA = resolve(process.env.SAIDA || 'target');
mkdirSync(SAIDA, { recursive: true });

const reprovas = [];
const nota = (ok, oQue, valor) => {
  console.log(`${ok ? 'ok ' : 'XX '} ${oQue}: ${valor}`);
  if (!ok) reprovas.push(oQue);
};
const dormir = ms => new Promise(r => setTimeout(r, ms));
const passo = t => console.log(`   · ${t}`);

const CARGA_LOGIN = 'joana.carga';
const CARGA_IP = '127.0.0.3';
const DB = 'aq707';
const TAB = 'Vendas_Blumenau';

function hash(senha) {
  const r = spawnSync(PHXSQLD, ['--senha'], { input: senha, encoding: 'utf8' });
  const m = /"senha_hash": "([^"]+)"/.exec(r.stdout || '');
  if (!m) throw new Error(`phxsqld --senha nao devolveu o hash: ${r.stdout}${r.stderr}`);
  return m[1];
}

/* Uma conexao de dados: uma linha de JSON por pedido, uma por resposta. */
async function conexao(porta, { de = '127.0.0.1', login = null } = {}) {
  const s = connect({ host: '127.0.0.1', port: porta, localAddress: de });
  s.setEncoding('utf8');
  s.on('error', () => {});
  await new Promise((res, rej) => { s.once('connect', res); s.once('error', rej); });
  let buf = ''; const espera = [];
  s.on('data', d => {
    buf += d; let i;
    while ((i = buf.indexOf('\n')) >= 0) {
      const l = buf.slice(0, i); buf = buf.slice(i + 1);
      if (l.trim() && espera.length) espera.shift()(JSON.parse(l));
    }
  });
  const pedir = o => new Promise(res => { espera.push(res); s.write(JSON.stringify({ token: TOKEN, ...o }) + '\n'); });
  if (login) {
    const r = await pedir({ op: 'login', usuario: login, senha: SENHA });
    if (!r.ok) throw new Error(`login ${login}: ${JSON.stringify(r)}`);
  }
  return { pedir, fechar: () => s.destroy() };
}

const h = hash(SENHA);
const servidor = await subir({
  phxsqld: PHXSQLD, portaDados: 6370, portaWeb: 6371, log: console.log,
  extra: {
    usuarios: [
      { id: 10, nome: 'Adriano Boller', login: USUARIO, senha_hash: h, supervisor: true, ativo: true, bases: {} },
      { id: 11, nome: 'Joana da Carga', login: CARGA_LOGIN, senha_hash: h, supervisor: true, ativo: true, bases: {} },
      { id: 12, nome: 'TV da parede', login: 'tv', senha_hash: h, supervisor: false, ativo: true,
        bases: { '*': { ler: true, monitorar: true } } },
    ],
    // limiares baixos para a carga de uma maquina de teste pintar as cores
    telemetria: { alto_uso_ms: 400, stress_ms: 1500 },
  },
});

let cargaViva = true;
const conexoesDeCarga = [];
const nav = await chromium.launch();
try {
  // ------------------------------------------------------------- o banco
  const adm = await conexao(6370, { login: USUARIO });
  const lig = await adm.pedir({ op: 'telemetria_ligar' });
  nota(lig.ok, 'telemetria ligada', JSON.stringify(lig.resultado || lig.erro).slice(0, 80));
  await adm.pedir({ op: 'criar_database', database: DB });
  const ct = await adm.pedir({ op: 'criar_tabela', database: DB, tabela: TAB, colunas: [
    { nome: 'id', tipo: 'Int4', obrigatoria: true },
    { nome: 'cidade', tipo: 'Str(30)' },
    { nome: 'valor', tipo: 'Int4' },
  ], indices: [{ nome: 'porId', colunas: ['id'], unico: true, primario: true }] });
  nota(ct.ok, 'tabela criada', ct.erro || TAB);
  // lotes de mil: a linha do protocolo tem teto de 64 KiB
  const tCarga = Date.now();
  for (let b = 0; b < 300; b++) {
    const linhas = Array.from({ length: 1000 }, (_, i) => ({ id: b * 1000 + i + 1, cidade: 'Blumenau', valor: i % 997 }));
    const r = await adm.pedir({ op: 'inserir_lote', database: DB, tabela: TAB, linhas });
    if (!r.ok) { nota(false, 'inserir_lote', r.erro); break; }
    if (b % 50 === 49) console.log(`   ${(b + 1) * 1000} linhas, ${Math.round((Date.now() - tCarga) / 1000)} s`);
  }

  // --------------------------------------------------------- a carga real
  const t0 = Date.now();
  const medir = await conexao(6370, { de: CARGA_IP, login: CARGA_LOGIN });
  const v = await medir.pedir({ op: 'agrupar', database: DB, tabela: TAB, por: ['valor'], agregados: [{ funcao: 'contagem', apelido: 'n' }] });
  nota(v.ok, 'agrupar de 300 mil linhas', `${Date.now() - t0} ms ${v.erro || ''}`);
  medir.fechar();
  for (let k = 0; k < 4; k++) {
    const c = await conexao(6370, { de: CARGA_IP, login: CARGA_LOGIN });
    conexoesDeCarga.push(c);
    (async () => {
      while (cargaViva) {
        await c.pedir({ op: 'agrupar', database: DB, tabela: TAB, por: ['valor'], agregados: [{ funcao: 'soma', coluna: 'id', apelido: 's' }] });
        await dormir(50 + 150 * k);
      }
    })();
  }

  await dormir(2500);
  const ret = await adm.pedir({ op: 'aquario_retrato' });
  const vivas = ret.ok ? ret.resultado.tarefas.filter(t => t.tabela === TAB) : [];
  nota(vivas.length > 0, 'a carga aparece no retrato', ret.ok
    ? JSON.stringify(ret.resultado).slice(0, 600) : ret.erro);

  adm.fechar();

  // TEMAS=escuro roda um tema so (a prova com o defeito reposto)
  for (const tema of (process.env.TEMAS || 'escuro,claro').split(',')) {
    console.log(`\n== tema ${tema}`);
    const ctx = await nav.newContext({ viewport: { width: 1600, height: 900 } });
    await ctx.route(u => !u.href.startsWith(servidor.url), r => r.abort());
    await ctx.addInitScript(t => { try { localStorage.setItem('phxsql-tema', t); } catch {} }, tema);
    const page = await ctx.newPage();
    const erros = [];
    page.on('console', m => { if (m.type() === 'error') erros.push(m.text()); });
    page.on('pageerror', e => erros.push(String(e)));
    // o ULTIMO retrato que a tela recebeu, e quantos pedidos do aquario sairam
    let ultimo = null, pedidosAq = 0;
    page.on('request', r => {
      if (r.url().endsWith('/api') && /"op":"aquario_/.test(r.postData() || '')) pedidosAq++;
    });
    page.on('response', async r => {
      if (!r.url().endsWith('/api')) return;
      const corpo = r.request().postData() || '';
      if (!corpo.includes('"op":"aquario_retrato"')) return;
      try { const j = await r.json(); if (j.ok) ultimo = j.resultado; } catch {}
    });

    await entrar(page, servidor.url + '?tela=aquario');
    await page.waitForSelector('#aqTela .aq-b', { timeout: 20000 });
    await dormir(4500);   // as bolhas crescem e a carga pinta as cores

    // -- cor: cada bolha viva com a cor da tarefa no ultimo retrato
    const cores = await page.evaluate(r => {
      const raiz = getComputedStyle(document.documentElement);
      const tinta = { verde: '--ok', azul_claro: '--reg', azul_escuro: '--aq-azul-escuro', amarelo: '--ambar',
                      vermelho: '--vermelho', rosa: '--acao-marcar' };
      const ver = document.createElement('span'); document.body.append(ver);
      const rgb = v => { ver.style.color = v; return getComputedStyle(ver).color; };
      const porId = new Map((r.tarefas || []).map(t => [t.tarefa, t.cor]));
      const out = [];
      for (const g of document.querySelectorAll('#aqTela .aq-b:not(.aq-fim)')) {
        const id = g.getAttribute('data-id');
        if (!porId.has(id)) continue;
        const forma = g.querySelector('.aq-forma');
        out.push({ id, dom: g.getAttribute('data-cor'), retrato: porId.get(id),
          stroke: getComputedStyle(forma).stroke, esperado: rgb(raiz.getPropertyValue(tinta[porId.get(id)]).trim()) });
      }
      ver.remove();
      return out;
    }, ultimo);
    const erradas = cores.filter(c => c.dom !== c.retrato || c.stroke !== c.esperado);
    const vistas = [...new Set(cores.map(c => c.retrato))].join(',');
    nota(cores.length > 0 && erradas.length === 0, `cor do retrato (${cores.length} bolhas: ${vistas})`,
      erradas.length ? JSON.stringify(erradas.slice(0, 3)) : 'todas batem');

    // -- faixas por cima das bolhas, e o dado cru
    const ordem = await page.evaluate(() => {
      const svg = document.querySelector('#aqTela .aq svg');
      const gs = [...svg.children].filter(n => n.tagName === 'g');
      const iNomes = gs.findIndex(g => g.querySelector('.aq-faixa-nome'));
      const iBolhas = gs.findIndex(g => g.querySelector('.aq-b'));
      const rot = [...document.querySelectorAll('#aqTela .aq-rot')].map(t => t.textContent).find(t => t) || '';
      const tit = [...document.querySelectorAll('#aqTela .aq-b:not(.aq-fim) title')].map(t => t.textContent).find(t => t) || '';
      const tt = [...document.querySelectorAll('#aqTela .aq-rot')].map(t => getComputedStyle(t).textTransform);
      return { iNomes, iBolhas, rot, tit, tt: [...new Set(tt)].join(',') };
    });
    nota(ordem.iNomes > ordem.iBolhas && ordem.iBolhas >= 0, 'nomes das faixas por cima das bolhas',
      `grupo dos nomes ${ordem.iNomes}, das bolhas ${ordem.iBolhas}`);
    // o rotulo corta o que nao cabe ANUNCIANDO (…), nunca muda a caixa; o
    // nome inteiro, como gravado, fica no <title>
    const prefixo = ordem.rot.replace(/…$/, '');
    nota(prefixo.length > 0 && TAB.startsWith(prefixo) && ordem.tit.includes(` ${TAB} `) && ordem.tt === 'none',
      'tabela como gravada', `«${ordem.rot}» / «${ordem.tit.slice(0, 60)}» text-transform=${ordem.tt}`);

    // -- inserir real sobe a barra do dia
    passo('inserir');
    const barra = () => page.evaluate(() => {
      const r = document.querySelector('.aqt-g[data-periodo="dia"] rect[data-serie="insert"]');
      return r ? +r.getAttribute('data-valor') : -1;
    });
    await dormir(4500);
    const antes = await barra();
    passo(`barra antes ${antes}`);
    // Pela propria tela (o `api` da pagina, o caminho do formulario): a
    // conexao de dados do preparo pode ter vencido por ociosidade enquanto o
    // navegador entrava. Espera na fila da trava atras da carga, e e real.
    const ins = await page.evaluate(([d, t, id]) => api('inserir', { database: d, tabela: t,
      valores: { id, cidade: 'Blumenau', valor: 1 } }).then(() => ({ ok: true }), e => ({ ok: false, erro: String(e) })),
      [DB, TAB, 900000 + (tema === 'claro' ? 1 : 0)]);
    passo(`inserir respondeu ${ins.ok}`);
    let depois = antes;
    for (let i = 0; i < 40 && depois <= antes; i++) { await dormir(250); depois = await barra(); }
    nota(ins.ok && antes >= 0 && depois === antes + 1, 'inserir sobe a barra do dia', `${antes} -> ${depois} ${ins.erro || ''}`);
    await page.screenshot({ path: join(SAIDA, `aquario-tela-${tema}.png`) });

    passo('contraste');
    // -- contraste dos rotulos
    const contraste = await page.evaluate(() => {
      const rgba = s => (s.match(/[\d.]+/g) || [0, 0, 0]).map(Number);
      const lum = c => { const f = v => { v /= 255; return v <= .03928 ? v / 12.92 : ((v + .055) / 1.055) ** 2.4; };
        return .2126 * f(c[0]) + .7152 * f(c[1]) + .0722 * f(c[2]); };
      const razao = (a, b) => { const [x, y] = [lum(a), lum(b)].sort((p, q) => q - p); return (x + .05) / (y + .05); };
      const fundoDe = el => { for (let e = el; e; e = e.parentElement) {
        const c = getComputedStyle(e).backgroundColor; const v = rgba(c);
        if (!(c.startsWith('rgba') && v[3] === 0)) return v; } return [255, 255, 255]; };
      const faixa = document.querySelector('#aqTela .aq-faixa-nome');
      const leg = document.querySelector('.aqt-leg li span:nth-child(2)');
      const log = document.querySelector('.aqt-lista li time') || document.querySelector('.aqt-nada');
      const sub = document.querySelector('.aqt-g .aqt-sub');
      return {
        faixa: razao(rgba(getComputedStyle(faixa).fill), fundoDe(faixa.closest('.aq'))),
        legenda: razao(rgba(getComputedStyle(leg).color), fundoDe(leg)),
        log: razao(rgba(getComputedStyle(log).color), fundoDe(log)),
        sub: razao(rgba(getComputedStyle(sub).color), fundoDe(sub)),
      };
    });
    for (const [k, r] of Object.entries(contraste)) nota(r >= 4.5, `contraste ${k}`, r.toFixed(2) + ':1');

    passo('log');
    // -- log: a linha do tempo tem eventos e a busca filtra
    const log = await page.evaluate(async () => {
      const n0 = document.querySelectorAll('.aqt-lista li[data-evento]').length;
      const busca = document.querySelector('.aqt-busca');
      busca.value = 'zzz-nada'; busca.dispatchEvent(new Event('input'));
      await new Promise(r => setTimeout(r, 300));
      const n1 = document.querySelectorAll('.aqt-lista li[data-evento]').length;
      busca.value = 'blumenau'; busca.dispatchEvent(new Event('input'));
      await new Promise(r => setTimeout(r, 300));
      const n2 = document.querySelectorAll('.aqt-lista li[data-evento]').length;
      busca.value = ''; busca.dispatchEvent(new Event('input'));
      return { n0, n1, n2 };
    });
    nota(log.n0 > 0 && log.n1 === 0 && log.n2 > 0, 'linha do tempo pesquisável', JSON.stringify(log));

    passo('oculta');
    // -- aba escondida = 0 pedidos (a aba de verdade: outra pagina na frente)
    const outra = await ctx.newPage();
    await outra.goto('about:blank').catch(() => {});
    await outra.bringToFront();
    await dormir(400);
    let vis = await page.evaluate(() => document.visibilityState);
    let como = 'outra aba na frente';
    if (vis !== 'hidden') {
      // O headless nao esconde a aba de tras: escondemos pelo proprio
      // documento, e o evento e o mesmo que o navegador dispararia.
      como = 'visibilityState forjado (o headless nao esconde a aba de tras)';
      await page.evaluate(() => {
        Object.defineProperty(document, 'hidden', { configurable: true, get: () => true });
        Object.defineProperty(document, 'visibilityState', { configurable: true, get: () => 'hidden' });
        document.dispatchEvent(new Event('visibilitychange'));
      });
      vis = 'hidden';
    }
    await dormir(600);   // o pedido que ja estava no ar termina
    const n0 = pedidosAq;
    await dormir(6000);
    const escondida = pedidosAq - n0;
    nota(escondida === 0, `aba escondida faz 0 pedidos (${como})`, `${escondida} em 6 s`);
    await page.bringToFront();
    await page.evaluate(() => {
      delete document.hidden; delete document.visibilityState;
      document.dispatchEvent(new Event('visibilitychange'));
    });
    const n1 = pedidosAq;
    await dormir(3000);
    nota(pedidosAq - n1 > 0, 'de volta a vista, volta a pedir', `${pedidosAq - n1} em 3 s`);
    await outra.close();

    // -- login e IP: o retrato do administrador os TEM
    const completo = ultimo && (ultimo.tarefas || []).some(t => t.usuario === CARGA_LOGIN && t.ip === CARGA_IP);
    const html = await page.evaluate(() => document.documentElement.outerHTML);
    nota(completo, 'o retrato do administrador traz login e IP (a prova tem o dado)', completo);
    nota(!html.includes(CARGA_LOGIN) && !html.includes(CARGA_IP), 'nem o administrador ve login/IP no aquario',
      `${html.includes(CARGA_LOGIN)}/${html.includes(CARGA_IP)}`);

    passo('alca');
    // -- a alca da telemetria
    await page.evaluate(() => PhxTelas.abrir('telemetria'));
    await page.waitForSelector('#aqAlca');
    const antesAlca = pedidosAq;
    await dormir(2500);
    nota(pedidosAq === antesAlca, 'alca fechada: 0 pedidos do aquario', `${pedidosAq - antesAlca}`);
    await page.click('#aqAlca > summary');
    await page.waitForSelector('#aqAlcaCaixa .aq-b', { timeout: 15000 });
    await page.evaluate(() => { document.querySelector('#aqAlcaCaixa').style.height = '520px'; });
    await dormir(800);
    const guardada = await page.evaluate(() => localStorage.getItem(`phxsql.aquario.altura.${(est.usuario && est.usuario.login) || '-'}`));
    nota(guardada === '520', 'alca estica e guarda a altura por usuario', guardada);
    await page.locator('#aqAlca').scrollIntoViewIfNeeded();
    await page.screenshot({ path: join(SAIDA, `aquario-alca-${tema}.png`) });
    await page.click('#aqAlca > summary');
    await dormir(600);
    const fechou = pedidosAq;
    await dormir(4000);
    nota(pedidosAq === fechou, 'alca fechada de novo: 0 pedidos', `${pedidosAq - fechou}`);

    nota(erros.length === 0, 'console sem erro (administrador)', erros.slice(0, 3).join(' | ') || 'nenhum');
    await ctx.close();

    passo('tv');
    // ------------------------------------------------- a TV: so monitorar
    const ctxTv = await nav.newContext({ viewport: { width: 1920, height: 1080 } });
    await ctxTv.route(u => !u.href.startsWith(servidor.url), r => r.abort());
    await ctxTv.addInitScript(t => { try { localStorage.setItem('phxsql-tema', t); } catch {} }, tema);
    const tv = await ctxTv.newPage();
    const errosTv = [];
    let retratoTv = null;
    tv.on('console', m => { if (m.type() === 'error') errosTv.push(m.text()); });
    tv.on('pageerror', e => errosTv.push(String(e)));
    const recusasTv = [];
    tv.on('response', r => {
      if (r.status() >= 400) recusasTv.push(`${r.status()} ${r.url().replace(servidor.url, '/')} ${(r.request().postData() || '').match(/"op":"[^"]+"/)?.[0] || ''}`);
    });
    tv.on('response', async r => {
      if (!r.url().endsWith('/api') || !(r.request().postData() || '').includes('"op":"aquario_retrato"')) return;
      try { const j = await r.json(); if (j.ok) retratoTv = j.resultado; } catch {}
    });
    await entrar(tv, servidor.url + '?tela=aquario', { usuario: 'tv', senha: SENHA, token: TOKEN });
    await tv.waitForSelector('#aqTela .aq-b', { timeout: 20000 });
    await dormir(5000);
    const htmlTv = await tv.evaluate(() => document.documentElement.outerHTML);
    const temCarga = retratoTv && (retratoTv.tarefas || []).some(t => t.tabela === TAB);
    const semCampo = retratoTv && (retratoTv.tarefas || []).every(t => !('usuario' in t) && !('ip' in t));
    nota(temCarga && semCampo, 'retrato da TV: tarefas da carga, sem usuario nem ip', `${temCarga}/${semCampo}`);
    nota(!htmlTv.includes(CARGA_LOGIN) && !htmlTv.includes(CARGA_IP), 'TV: login e IP fora do DOM',
      `${htmlTv.includes(CARGA_LOGIN)}/${htmlTv.includes(CARGA_IP)}`);
    await tv.screenshot({ path: join(SAIDA, `aquario-tv-${tema}.png`) });
    // A TV entra sem direito de administrar: o Painel que o login abre por
    // baixo pode recusar, e isso aparece como aviso na tela, nao no console.
    nota(errosTv.length === 0, 'console sem erro (TV)', (errosTv.slice(0, 3).join(' | ') || 'nenhum') + ' ' + recusasTv.join(' | '));
    await ctxTv.close();
  }
} catch (e) {
  nota(false, 'excecao', e && e.stack || e);
} finally {
  cargaViva = false;
  await dormir(300);
  for (const c of conexoesDeCarga) c.fechar();
  await nav.close();
  await servidor.derrubar();
}
console.log(`\n${reprovas.length ? 'REPROVOU: ' + reprovas.join('; ') : 'tudo verde'}`);
process.exit(reprovas.length ? 1 : 0);
