// Veredito de qualificacao da interface, tela por tela, com o MESMO criterio do relatorio
// (docs/ui/qualificacao/RELATORIO_2026-10-01.md):
//
//   NAO QUALIFICADA  algum «bloqueia» ou «grave» no caminho principal da tela;
//   COM RESSALVAS    os graves (ou bloqueios) que sobram sao so a 390 numa tela de mesa, ou
//                    fora do caminho principal;
//   QUALIFICADA      nenhum dos dois.
//
// Cada achado do relatorio vira uma SONDA que mede o defeito exercitando a pagina num
// Chromium (interface so se prova exercitando): nenhuma sonda le o fonte para decidir. Os
// medios e cosmeticos tambem se medem e aparecem ao lado, mas nao entram no veredito.
//
// Uso: PLAYWRIGHT_BROWSERS_PATH=/opt/pw-browsers node tests/desktop/qualificacao/qualificar.mjs
//        [--ui DIR]        arvore a medir (padrao apps/phxclaw-ui); RED = copia com o conserto desfeito
//        [--so B1,G7,...]  so estas sondas; sai 1 se alguma reprovar
// Sai 1 se alguma tela nao estiver QUALIFICADA. Grava tests/desktop/out/qualificacao/qualificar.json.
import { readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { subir } from './servidor.mjs';
import { stubTauriFn } from './stub.mjs';
import { chromium, UI, OUT, CAP, MEDIR, GRADE, CONFIG, TELAS, RAIZ } from './comum.mjs';

const iSo = process.argv.indexOf('--so');
const SO = iSo > 0 ? new Set(process.argv[iSo + 1].split(',')) : null;
const quer = id => !SO || SO.has(id);
const FAB = JSON.parse(readFileSync(join(UI, 'assets/textos.json'), 'utf8')).textos;

// Telas do veredito. «mesa»: tela de trabalho de mesa, onde um defeito so a 390 e ressalva;
// Tarefas, a barra e o PWA sao o caminho do celular (start_url do aplicativo instalavel).
const QTELAS = {
  splash: { nome: 'Splash', mesa: false }, topo: { nome: 'Barra do topo / navegacao', mesa: false },
  geral: { nome: 'Visao geral', mesa: true }, agentes: { nome: 'Agentes', mesa: true }, ide: { nome: 'IDE', mesa: true },
  ferramentas: { nome: 'Ferramentas', mesa: true }, absorcao: { nome: 'Absorcao', mesa: true },
  tarefas: { nome: 'Tarefas', mesa: false }, config: { nome: 'Configuracao', mesa: true },
  host: { nome: 'Painel do host', mesa: true }, rodape: { nome: 'Rodape', mesa: true }, offline: { nome: 'PWA sem rede', mesa: false },
};

const achados = [];
// sev: bloqueia | grave | medio | cosmetico. so390: o defeito so existe a 390.
// principal: false quando o defeito esta fora do caminho principal da tela.
function achado(id, sev, telas, ok, evid, { so390 = false, principal = true } = {}) {
  achados.push({ id, sev, telas, ok: !!ok, evid, so390, principal });
  console.log(`${ok ? 'ok   ' : 'FALHA'} ${id.padEnd(4)} ${sev.padEnd(9)} ${telas.join(',').padEnd(22)} ${String(evid).slice(0, 220)}`);
}

const { srv, porta, modo } = await subir(UI, { config: CONFIG });
const ORIG = `http://127.0.0.1:${porta}`;
const browser = await chromium.launch();

async function pagina({ w = 1366, h = null, lang = 'pt', tauri = true, token = true, sw = 'block', reducedMotion = null } = {}) {
  const ctx = await browser.newContext({
    viewport: { width: w, height: h || (w === 390 ? 844 : w === 1920 ? 1080 : w === 768 ? 1024 : 768) },
    locale: 'pt-BR', isMobile: w === 390, hasTouch: w <= 768, serviceWorkers: sw, reducedMotion: reducedMotion || 'no-preference',
  });
  if (tauri) await ctx.addInitScript(stubTauriFn, GRADE);
  await ctx.addInitScript(([l, t]) => { try { if (t) localStorage.setItem('phxclaw.token', 'token-de-teste'); localStorage.setItem('phxclaw.idioma', l); } catch {} }, [lang, token]);
  const page = await ctx.newPage();
  page.__erros = [];
  page.on('pageerror', e => page.__erros.push(String(e)));
  return { ctx, page };
}
async function abrirTela(page, tela, espera = 900) {
  await page.goto(`${ORIG}/index.html?screen=dashboard#${tela}`);
  await page.evaluate(() => (typeof idiomas !== 'undefined' ? idiomas.pronto : null)).catch(() => {});
  await page.evaluate(t => { if (location.hash !== `#${t}`) location.hash = t; }, tela);
  await page.waitForTimeout(espera);
}
// Uma sonda pode medir varios achados de uma vez («M4|M5»): roda se algum deles foi pedido.
const sonda = async (id, f) => {
  if (!id.split('|').some(quer)) return;
  try { await f(); } catch (e) { achado(id.split('|')[0], 'grave', ['?'], false, `sonda quebrou: ${String(e).split('\n')[0]}`); }
};
// Altura das linhas de DADO de uma grade (sem grupo, rodape, detalhe), so as visiveis.
const linhasDaGrade = (page, sel) => page.$$eval(`${sel} .phx-tabela > tbody > tr`, trs => trs
  .filter(tr => !/(^|\s)phx-(grupo|grupo-rodape|detalhe|tr-vazia|preview)(\s|$)/.test(tr.className) && tr.getClientRects().length)
  .map(tr => tr.getBoundingClientRect().height));
const mediana = a => { const s = [...a].sort((x, y) => x - y); return s.length ? s[Math.floor(s.length / 2)] : 0; };

/* ============================ BLOQUEIA ============================ */
await sonda('B1', async () => {
  const r = [];
  for (const w of [1366, 390]) {
    const { ctx, page } = await pagina({ w });
    await abrirTela(page, 'tarefas', 1200);
    await page.evaluate(() => { document.activeElement?.blur(); window.scrollTo(0, 0); });
    let dentro = false;
    for (let i = 0; i < 90 && !dentro; i++) {
      await page.keyboard.press('Tab');
      dentro = await page.evaluate(() => !!document.activeElement?.closest('#tarefasLista'));
    }
    const abre = async tecla => {
      await page.keyboard.press('ArrowDown');
      await page.waitForTimeout(150);
      const alvo = await page.evaluate(() => {
        const tr = document.activeElement?.closest('tbody tr');
        return tr ? tr.querySelector('td[data-tag="objetivo"]')?.textContent ?? null : null;
      });
      await page.keyboard.press(tecla);
      await page.waitForTimeout(900);
      const titulo = await page.evaluate(() => { const d = document.getElementById('tarefasDetalhe'); return d && !d.hidden ? d.querySelector('.tarefa-titulo')?.textContent : null; });
      return { alvo, titulo, ok: !!alvo && titulo === alvo };
    };
    const enter = dentro ? await abre('Enter') : null;
    const espaco = dentro ? await abre(' ') : null;
    r.push({ w, dentro, enter: enter?.ok, espaco: espaco?.ok });
    await ctx.close();
  }
  achado('B1', 'bloqueia', ['tarefas'], r.every(x => x.dentro && x.enter && x.espaco), JSON.stringify(r));
});

await sonda('B2', async () => {
  const { ctx, page } = await pagina({ w: 390 });
  const med = {};
  for (const t of ['tarefas', 'agentes', 'ferramentas', 'absorcao', 'config']) {
    await abrirTela(page, t, t === 'config' ? 1500 : 1000);
    const alvo = { tarefas: '#tarefasLista', agentes: '#agentesConteudo', ferramentas: '#ferramentasConteudo', absorcao: '#absorcaoConteudo', config: '#configConteudo' }[t];
    const hs = await linhasDaGrade(page, alvo);
    med[t] = { mediana: Math.round(mediana(hs)), n: hs.length, max: Math.round(Math.max(0, ...hs)) };
    await page.screenshot({ path: join(CAP, `q_B2_${t}_390.png`) });
    if (t === 'tarefas') {
      // Com o detalhe aberto, a lista nao pode cobrir os botoes dele: o cartao que vazava
      // da caixa da lista tomava o toque do RESPONDER (achado pelo pwa_ponte.mjs no lote 4).
      await page.click('#tarefasLista tbody tr[data-id="t2"]');
      await page.waitForTimeout(900);
      med[t].tocavel = await page.evaluate(() => {
        const b = document.querySelector('#tarefasDetalhe .tarefa-pergunta button');
        if (!b) return false;
        b.scrollIntoView({ block: 'center' });
        const r = b.getBoundingClientRect();
        return document.elementFromPoint(r.left + r.width / 2, r.top + r.height / 2) === b;
      });
    }
  }
  await ctx.close();
  for (const [t, m] of Object.entries(med)) {
    achado('B2', 'bloqueia', [t], m.n > 0 && m.mediana <= 120 && m.tocavel !== false, `390: altura mediana da linha ${m.mediana} px (max ${m.max}, ${m.n} linhas); prova <= 120${t === 'tarefas' ? `; RESPONDER tocavel com o detalhe aberto: ${m.tocavel}` : ''}`, { so390: true });
  }
});

/* ============================ GRAVE ============================ */
await sonda('G1', async () => {
  const { ctx, page } = await pagina({ w: 390 });
  await abrirTela(page, 'geral');
  const m = await page.evaluate(() => {
    const W = innerWidth;
    const nav = document.querySelector('.sidebar nav').getBoundingClientRect();
    const botoes = [...document.querySelectorAll('.nav[data-tela]')].map(b => { const r = b.getBoundingClientRect(); return { t: b.dataset.tela, l: Math.round(r.left), r: Math.round(r.right), w: Math.round(r.width), h: Math.round(r.height) }; });
    const topo = [...document.querySelectorAll('.topbar *')].filter(e => e.getClientRects().length && getComputedStyle(e).visibility !== 'hidden')
      .map(e => [e.className || e.tagName, Math.round(e.getBoundingClientRect().right)]).filter(([, r]) => r > W + 0.5);
    return { W, navR: Math.round(nav.right), fora: botoes.filter(b => b.l < -0.5 || b.r > W + 0.5 || b.w < 44 || b.h < 44), topo };
  });
  await page.screenshot({ path: join(CAP, 'q_G1_390.png') });
  await ctx.close();
  achado('G1', 'grave', ['topo'], m.navR <= m.W && !m.fora.length && !m.topo.length, JSON.stringify(m));
});

await sonda('G2', async () => {
  const { ctx, page } = await pagina({ w: 390 });
  await abrirTela(page, 'geral', 1200);
  const m = await page.evaluate(() => {
    const t = document.getElementById('tela-geral');
    const fora = [];
    for (const card of document.querySelectorAll('#tela-geral .metric, #tela-geral .panel')) {
      const c = card.getBoundingClientRect();
      for (const e of card.querySelectorAll('*')) {
        if (!e.getClientRects().length || e.closest('.event-console')) continue;
        const r = e.getBoundingClientRect();
        if (r.width && r.right > c.right + 1) { fora.push(`${e.tagName.toLowerCase()}.${e.className}`.slice(0, 40)); break; }
      }
    }
    return { sw: t.scrollWidth, cw: t.clientWidth, fora };
  });
  await ctx.close();
  achado('G2', 'grave', ['geral'], m.sw <= m.cw + 1 && !m.fora.length, JSON.stringify(m), { so390: true });
});

await sonda('G3', async () => {
  const r = [];
  for (const w of [1920, 1366, 768, 390]) {
    const { ctx, page } = await pagina({ w });
    await page.goto(`${ORIG}/index.html?screen=splash`);
    await page.waitForTimeout(4200);
    const m = await page.evaluate(() => {
      const pe = document.querySelector('.splash-foot')?.getBoundingClientRect();
      let area = 0;
      for (const s of document.querySelectorAll('.boot-modules > *')) {
        const b = s.getBoundingClientRect();
        if (!pe) break;
        const x = Math.min(pe.right, b.right) - Math.max(pe.left, b.left), y = Math.min(pe.bottom, b.bottom) - Math.max(pe.top, b.top);
        if (x > 0 && y > 0) area += x * y;
      }
      const fora = [...document.querySelectorAll('.splash-center *')].filter(e => e.getClientRects().length)
        .filter(e => { const b = e.getBoundingClientRect(); return b.right > innerWidth + 1 || b.left < -1; }).map(e => e.className || e.tagName);
      return { area: Math.round(area), fora: [...new Set(fora)].slice(0, 4) };
    });
    await page.screenshot({ path: join(CAP, `q_G3_splash_${w}.png`) });
    r.push({ w, ...m });
    await ctx.close();
  }
  achado('G3', 'grave', ['splash'], r.every(x => x.area === 0 && !x.fora.length), JSON.stringify(r));
});

await sonda('G4', async () => {
  const { ctx, page } = await pagina({ w: 1366 });
  // A fabrica chega devagar: o splash fica na tela enquanto a pagina nao esta pronta, e e
  // nessa janela que o Tab nao pode cair em controle invisivel nem no app escondido.
  await page.route('**/assets/textos.json', async rt => { await new Promise(x => setTimeout(x, 2500)); rt.continue(); });
  await page.goto(`${ORIG}/index.html`);
  await page.waitForTimeout(250);
  const seq = [];
  for (let i = 0; i < 6; i++) {
    await page.keyboard.press('Tab');
    seq.push(await page.evaluate(() => {
      const sp = document.getElementById('splash');
      const cobre = sp && getComputedStyle(sp).visibility !== 'hidden' && +getComputedStyle(sp).opacity > 0 && getComputedStyle(sp).display !== 'none';
      const e = document.activeElement;
      if (!e || e === document.body) return { cobre, foco: null };
      let op = 1;
      for (let n = e; n && n.nodeType === 1; n = n.parentElement) op *= +getComputedStyle(n).opacity;
      return { cobre, foco: e.id || e.className || e.tagName, invisivel: op < 0.05, noApp: !!e.closest('#app'), ariaHidden: !!e.closest('[aria-hidden="true"]') };
    }));
  }
  await ctx.close();
  const durante = seq.filter(s => s.cobre);
  const ruins = durante.filter(s => s.foco && (s.invisivel || s.noApp || s.ariaHidden));
  achado('G4', 'grave', ['splash'], durante.length > 0 && !ruins.length,
    durante.length ? `${durante.length} Tabs com o splash na tela; ruins: ${JSON.stringify(ruins.slice(0, 3))}` : 'splash sumiu antes do primeiro Tab: nao medido');
});

await sonda('G5', async () => {
  const r = {};
  for (const tauri of [false, true]) {
    const { ctx, page } = await pagina({ w: 1366, tauri });
    await abrirTela(page, 'geral', 900);
    r[tauri ? 'host' : 'sem_host'] = await page.evaluate(() => {
      const topo = document.querySelector('.topbar').innerText.replace(/\s+/g, ' ');
      // O ponto que mente e o que PULSA (animacao rodando) ou acende verde sem host.
      const ponto = [...document.querySelectorAll('.topbar .pulse-dot')].filter(e => e.getClientRects().length && getComputedStyle(e).display !== 'none')
        .filter(e => e.getAnimations().some(a => a.playState === 'running') || /rgb\(87, 230, 168\)/.test(getComputedStyle(e).backgroundColor)).length;
      return { topo, ponto };
    });
    await ctx.close();
  }
  const mente = /\bONLINE\b|\bCONNECTED\b|\bLIVE\b|CONECTADO|NO AR/i;
  const s = r.sem_host;
  achado('G5', 'grave', ['topo'], !mente.test(s.topo) && s.ponto === 0, `sem host: «${s.topo.slice(0, 120)}» pontos pulsando=${s.ponto}`);
});

await sonda('G6', async () => {
  const { ctx, page } = await pagina({ w: 1366 });
  await abrirTela(page, 'geral', 900);
  const ids = await page.$$eval('.topbar button', bs => bs.map((b, i) => { b.dataset.qi = String(i); return b.id; }));
  const r = [];
  for (let i = 0; i < ids.length; i++) {
    if (ids[i] === 'trocarIdioma') continue;
    const sel = `.topbar button[data-qi="${i}"]`;
    const nome = await page.$eval(sel, b => (b.getAttribute('aria-label') || b.getAttribute('title') || b.textContent || '').trim());
    const antes = await page.evaluate(() => [...document.querySelectorAll('[role="dialog"],[role="search"],dialog,.paleta,.busca-global')].filter(e => e.getClientRects().length && !e.hidden).length);
    await page.click(sel);
    await page.waitForTimeout(250);
    const depois = await page.evaluate(() => [...document.querySelectorAll('[role="dialog"],[role="search"],dialog,.paleta,.busca-global')].filter(e => e.getClientRects().length && !e.hidden).length);
    // Busca de verdade: o nome de uma ferramenta (dado do JSON gerado), Enter, e a tela
    // dela abre filtrada por ele.
    let acha = null;
    if (depois > antes && await page.$('dialog[open] input')) {
      const nomeF = JSON.parse(readFileSync(join(UI, 'assets/ferramentas.json'), 'utf8')).ferramentas[0].nome;
      await page.keyboard.type(nomeF);
      await page.waitForTimeout(300);
      await page.keyboard.press('Enter');
      await page.waitForTimeout(400);
      acha = await page.evaluate(n => document.body.dataset.tela === 'ferramentas' && document.getElementById('ferramentasFiltro').value === n, nomeF);
    } else {
      await page.keyboard.press('Escape');
    }
    await page.waitForTimeout(150);
    r.push({ nome: nome.slice(0, 30), nomeOk: /\p{L}{2}/u.test(nome), faz: depois > antes, acha });
  }
  await ctx.close();
  achado('G6', 'grave', ['topo'], r.every(x => x.nomeOk && x.faz && x.acha !== false), JSON.stringify(r));
});

await sonda('G7', async () => {
  // Jornada A do relatorio: primeira visita com rede (o SW instala a casca) e a rede cai.
  const s2 = await subir(UI, { config: CONFIG });
  const O2 = `http://localhost:${s2.porta}`;
  const ctx = await browser.newContext({ viewport: { width: 390, height: 844 }, locale: 'pt-BR', isMobile: true, hasTouch: true });
  await ctx.addInitScript(() => { try { localStorage.setItem('phxclaw.token', 'token-de-teste'); } catch {} });
  const page = await ctx.newPage();
  await page.goto(`${O2}/?tela=tarefas`);
  await page.evaluate(() => navigator.serviceWorker.ready);
  await page.waitForTimeout(1500);
  const cache = await page.evaluate(async () => { const r = []; for (const k of await caches.keys()) r.push(...(await (await caches.open(k)).keys()).map(q => new URL(q.url).pathname)); return r; });
  s2.srv.closeAllConnections?.(); await new Promise(r => s2.srv.close(r));
  await page.reload().catch(() => {});
  await page.waitForTimeout(2500);
  await page.evaluate(() => { location.hash = 'agentes'; }).catch(() => {});
  await page.waitForTimeout(900);
  const agentes = await page.evaluate(() => document.getElementById('agentesResumo')?.textContent ?? '').catch(e => `ERRO ${e.message}`);
  const fonte = await page.evaluate(async () => { await document.fonts.ready; return [...document.fonts].filter(f => f.family.includes('Exo 2')).map(f => f.status).join(','); }).catch(() => '?');
  await page.screenshot({ path: join(CAP, 'q_G7_offline_agentes.png') });
  await ctx.close();
  const semCache = ['/assets/equipe.json', '/assets/ferramentas.json', '/assets/absorcao.json', '/assets/fonte/exo2-latin.woff2'].filter(x => !cache.includes(x));
  // E o 404 nao se confunde com a rede: falha de rede num JSON nao manda rodar cargo.
  const { ctx: c3, page: p3 } = await pagina({ w: 1366 });
  await p3.route('**/assets/equipe.json', rt => rt.abort('internetdisconnected'));
  await abrirTela(p3, 'agentes', 900);
  const rede = await p3.textContent('#agentesResumo');
  await c3.close();
  const ok = !semCache.length && !/cargo|Gere com/i.test(agentes) && /loaded/.test(fonte) && !/cargo|Gere com/i.test(rede);
  achado('G7', 'grave', ['offline'], ok, `sem cache: [${semCache.join(' ')}]; agentes offline: «${agentes.slice(0, 70)}»; fonte: ${fonte}; rede caida num JSON: «${rede.slice(0, 70)}»`);
});

await sonda('G8', async () => {
  const r = [];
  for (const lang of ['pt', 'en']) {
    for (const falha of ['rede', '500']) {
      for (const t of ['tarefas', 'config']) {
        const { ctx, page } = await pagina({ w: 1366, lang });
        await page.route('**/v1/**', rt => (falha === 'rede' ? rt.abort('internetdisconnected')
          : rt.fulfill({ status: 500, contentType: 'application/json', body: '{"error":"falha interna simulada"}' })));
        await abrirTela(page, t, 1200);
        const st = `#${t}Status`;
        const m = await page.evaluate(sel => {
          const s = document.querySelector(sel);
          const papel = s.getAttribute('role') || s.getAttribute('aria-live') || '';
          const sec = s.closest('section.tela');
          const tentar = [...sec.querySelectorAll('[data-tentar]')].filter(b => b.getClientRects().length && !b.hidden);
          return { texto: s.textContent.trim(), papel, tentar: tentar.length };
        }, st);
        // TENTAR DE NOVO tem de refazer o pedido: conta os pedidos a /v1/ que saem do clique
        // (o da sondagem de Tarefas, a cada 2,5 s, nao cabe nos 400 ms medidos).
        let refez = false;
        if (m.tentar) {
          let n = 0;
          page.on('request', q => { if (q.url().includes('/v1/')) n++; });
          await page.click(`#tela-${t} [data-tentar]`);
          await page.waitForTimeout(400);
          refez = n > 0;
        }
        const chave = falha === 'rede' ? 'erro.sem_rede' : null;
        const textoOk = !/Failed to fetch|NetworkError|TypeError|Load failed/.test(m.texto)
          && (lang === 'en' ? !/Falhou|Sem conex/.test(m.texto) : !/Failed|No connection/.test(m.texto))
          && (!chave || (FAB[chave] && m.texto.startsWith(FAB[chave][lang].split('{')[0].trim())));
        r.push({ lang, falha, t, ok: textoOk && /alert|status|assertive|polite/.test(m.papel) && m.tentar > 0 && refez, texto: m.texto.slice(0, 50), papel: m.papel, tentar: m.tentar, refez });
        await ctx.close();
      }
    }
  }
  for (const t of ['tarefas', 'config']) {
    const rr = r.filter(x => x.t === t);
    achado('G8', 'grave', [t], rr.every(x => x.ok), rr.every(x => x.ok) ? `${rr.length} casos (PT/EN x rede/500): mensagem pela chave, regiao viva, TENTAR DE NOVO refaz o pedido` : JSON.stringify(rr.filter(x => !x.ok).slice(0, 2)));
  }
});

await sonda('G9', async () => {
  const { ctx, page } = await pagina({ w: 1366 });
  await abrirTela(page, 'config', 1600);
  const cdp = await ctx.newCDPSession(page);
  const { nodes } = await cdp.send('Accessibility.getFullAXTree');
  const semNome = {};
  for (const n of nodes) {
    if (n.ignored) continue;
    const role = n.role?.value, nome = String(n.name?.value ?? '').trim();
    if (['textbox', 'checkbox', 'spinbutton', 'combobox', 'searchbox'].includes(role) && !nome) semNome[role] = (semNome[role] || 0) + 1;
  }
  const editores = await page.$$eval('#configConteudo .cfg-ed, #configConteudo .cfg-tag-in', e => e.length);
  await ctx.close();
  const n = Object.values(semNome).reduce((a, x) => a + x, 0);
  achado('G9', 'grave', ['config'], editores > 0 && n === 0, `${n} controles sem nome acessivel ${JSON.stringify(semNome)} de ${editores} editores`);
});

await sonda('G10', async () => {
  const { ctx, page } = await pagina({ w: 1366 });
  await abrirTela(page, 'ide', 600);
  await page.click('#ideAbrirBash');
  await page.waitForTimeout(400);
  // Chega ao canvas pelo teclado (o anel que conta e o do :focus-visible).
  await page.focus('#ideFechar');
  let noCanvas = false;
  for (let i = 0; i < 8 && !noCanvas; i++) { await page.keyboard.press('Tab'); noCanvas = await page.evaluate(() => document.activeElement?.id === 'termCanvas'); }
  const anel = await page.evaluate(() => {
    const cs = getComputedStyle(document.getElementById('termCanvas'));
    const w = cs.outlineStyle !== 'none' ? parseFloat(cs.outlineWidth) : 0;
    const m = cs.outlineColor.match(/\d+(\.\d+)?/g)?.map(Number) || [0, 0, 0];
    const lin = c => { c /= 255; return c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4; };
    const L = ([r, g, b]) => 0.2126 * lin(r) + 0.7152 * lin(g) + 0.0722 * lin(b);
    const contraste = (L(m) + 0.05) / (L([1, 4, 24]) + 0.05);
    const ajuda = (document.getElementById('termCanvas').getAttribute('aria-describedby') || '').split(/\s+/).map(id => document.getElementById(id)?.textContent || '').join(' ');
    return { largura: w, contraste: +contraste.toFixed(2), ajuda };
  });
  // Tab sozinho e do terminal (completa no bash); o atalho de saida e Esc e depois Tab.
  const n0 = await page.evaluate(() => window.__chamadas.length);
  await page.keyboard.press('Tab');
  const tabFoi = await page.evaluate(n => window.__chamadas.slice(n).some(c => c.cmd === 'terminal_escrever' && c.args.tecla?.tecla === 'Tab'), n0);
  const ficou = await page.evaluate(() => document.activeElement?.id);
  await page.keyboard.press('Escape');
  await page.keyboard.press('Tab');
  const saiu = await page.evaluate(() => document.activeElement?.id || document.activeElement?.tagName);
  await ctx.close();
  const ok = noCanvas && anel.largura >= 2 && anel.contraste >= 3 && /Esc/.test(anel.ajuda) && tabFoi && ficou === 'termCanvas' && saiu !== 'termCanvas';
  achado('G10', 'grave', ['ide'], ok, JSON.stringify({ noCanvas, ...anel, tabNoTerminal: tabFoi, escTabSai: saiu !== 'termCanvas' }), { principal: false });
});

await sonda('G11', async () => {
  const r = {};
  for (const w of [1366, 390]) {
    const { ctx, page } = await pagina({ w });
    await abrirTela(page, 'geral', 1200);
    r[w] = await page.evaluate(() => {
      const conta = { geral: [0, 0], topo: [0, 0] };
      const ex = {};
      const w = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT);
      for (let n = w.nextNode(); n; n = w.nextNode()) {
        const e = n.parentElement;
        if (!n.textContent.trim() || !e) continue;
        const onde = e.closest('#tela-geral') ? 'geral' : e.closest('.topbar, .sidebar, .footerbar') ? 'topo' : null;
        if (!onde) continue;
        let vis = !!e.getClientRects().length;
        for (let x = e; vis && x && x.nodeType === 1; x = x.parentElement) { const c = getComputedStyle(x); if (c.display === 'none' || c.visibility === 'hidden' || +c.opacity === 0) vis = false; }
        if (!vis) continue;
        const px = parseFloat(getComputedStyle(e).fontSize);
        conta[onde][1]++;
        if (px < 12) { conta[onde][0]++; const k = `${px}px ${e.tagName.toLowerCase()}.${e.className}`.slice(0, 40); ex[k] = (ex[k] || 0) + 1; }
      }
      return { conta, ex: Object.entries(ex).sort((a, b) => b[1] - a[1]).slice(0, 5) };
    });
    await ctx.close();
  }
  // Escala fixa (Style Phoenix Padrao, lote 8): todo texto visivel de todas as telas num dos
  // oito degraus; a marca desenhada do splash (.wordmark) e logotipo, nao texto de leitura.
  const ESCALA = new Set([12, 13, 15, 18, 21, 25, 30, 37]);
  const fora = {};
  for (const w of [1366, 390]) {
    const { ctx, page } = await pagina({ w });
    for (const t of TELAS) {
      await abrirTela(page, t, t === 'config' ? 1400 : 800);
      const tam = await page.evaluate(() => {
        const r = {};
        const w = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT);
        for (let n = w.nextNode(); n; n = w.nextNode()) {
          const e = n.parentElement;
          if (!n.textContent.trim() || !e || e.closest('.wordmark, #splash, dialog:not([open])')) continue;
          let vis = !!e.getClientRects().length;
          for (let x = e; vis && x && x.nodeType === 1; x = x.parentElement) { const c = getComputedStyle(x); if (c.display === 'none' || c.visibility === 'hidden' || +c.opacity === 0) vis = false; }
          if (!vis) continue;
          const px = parseFloat(getComputedStyle(e).fontSize);
          const k = `${px}|${e.tagName.toLowerCase()}.${String(e.className).split(' ')[0]}`;
          r[k] = (r[k] || 0) + 1;
        }
        return r;
      });
      for (const [k, n] of Object.entries(tam)) if (!ESCALA.has(+k.split('|')[0])) fora[`${w}:${t}:${k}`] = n;
    }
    await ctx.close();
  }
  const nFora = Object.values(fora).reduce((a, x) => a + x, 0);
  achado('G11', 'grave', ['geral'], nFora === 0, `escala fixa ${[...ESCALA].join('/')}: ${nFora} nos de texto fora dela ${JSON.stringify(Object.entries(fora).slice(0, 60))}`, { principal: false });
  for (const t of ['geral', 'topo']) {
    const n = [1366, 390].map(w => r[w].conta[t][0]);
    achado('G11', 'grave', [t], n.every(x => x === 0), `nos de texto < 12 px: 1366=${r[1366].conta[t][0]}/${r[1366].conta[t][1]} 390=${r[390].conta[t][0]}/${r[390].conta[t][1]} ${JSON.stringify(r[1366].ex.slice(0, 3))}`, { principal: false });
  }
});

/* ============================ MEDIO e COSMETICO ============================ */
await sonda('M1|M2|M3', async () => {
  // M1-M3: contraste de todo texto visivel, todas as telas, 1366 e 390. A hachura de «pela
  // metade» e falso positivo medido (fundo por gradiente; cap/zoom_pela_metade.png do relatorio).
  const falhas = {};
  for (const w of [1366, 390]) {
    const { ctx, page } = await pagina({ w });
    for (const t of TELAS) {
      await abrirTela(page, t, t === 'config' ? 1400 : 800);
      const m = await page.evaluate(`(${MEDIR})()`);
      // A hachura de «pela metade» e o falso positivo medido do relatorio: o fundo sai da
      // media do gradiente (aprox) e a hachura so pinta a borda (cap/zoom_pela_metade.png).
      const metade = new Set([FAB['absorcao.parcial'].pt, FAB['absorcao.parcial'].en]);
      for (const c of m.contraste.filter(c => !(c.aprox && metade.has(c.texto)))) {
        const k = `${t}: ${c.sel} «${c.texto.slice(0, 16)}» ${c.razao}:1`;
        falhas[k] = (falhas[k] || 0) + 1;
      }
    }
    await ctx.close();
  }
  const ks = Object.keys(falhas);
  achado('M1', 'medio', ['rodape', 'tarefas', 'splash'], ks.length === 0, `${ks.length} textos abaixo do minimo: ${ks.slice(0, 12).join(' | ')}`);
  const { ctx, page } = await pagina({ w: 1366 });
  await page.goto(`${ORIG}/index.html?screen=splash`);
  await page.waitForTimeout(1500);
  const sp = (await page.evaluate(`(${MEDIR})()`)).contraste;
  await ctx.close();
  achado('M2', 'medio', ['splash'], sp.length === 0, JSON.stringify(sp.slice(0, 3).map(c => [c.sel, c.razao])));
});

await sonda('M4|M5', async () => {
  // Desempenho percebido no celular (CPU 4x, 4G lenta), mediana de 3: a 1a tela util do
  // start_url do PWA e o DOMContentLoaded da Visao geral (o phx-grid sincrono custava +2,7 s).
  const REDE4G = { latency: 150, downloadThroughput: 1.6 * 1024 * 1024 / 8, uploadThroughput: 750 * 1024 / 8 };
  const medir = async (url, celular = true) => {
    const runs = [];
    for (let i = 0; i < 3; i++) {
      const ctx = await browser.newContext({ viewport: celular ? { width: 390, height: 844 } : { width: 1366, height: 768 }, serviceWorkers: 'block' });
      await ctx.addInitScript(() => {
        try { localStorage.setItem('phxclaw.token', 't'); } catch {}
        const marca = () => { const a = document.getElementById('app'); const s = document.getElementById('splash');
          const appVis = a && (a.classList.contains('visible') || document.body.classList.contains('force-dashboard')) && getComputedStyle(a).opacity > 0.5;
          const splashFora = !s || getComputedStyle(s).visibility === 'hidden' || getComputedStyle(s).display === 'none' || s.classList.contains('hidden');
          if (appVis && splashFora && !window.__util) window.__util = performance.now(); };
        new MutationObserver(marca).observe(document, { attributes: true, subtree: true, childList: true });
        const loop = () => { marca(); if (!window.__util) requestAnimationFrame(loop); }; requestAnimationFrame(loop);
      });
      const page = await ctx.newPage();
      const cdp = await ctx.newCDPSession(page);
      await cdp.send('Network.enable');
      if (celular) {
        await cdp.send('Emulation.setCPUThrottlingRate', { rate: 4 });
        await cdp.send('Network.emulateNetworkConditions', { offline: false, ...REDE4G });
      }
      await page.goto(`${ORIG}${url}`, { waitUntil: 'load' });
      await page.waitForTimeout(url.includes('tarefas') ? 9000 : celular ? 1500 : 4500);
      runs.push(await page.evaluate(() => ({ dcl: Math.round(performance.getEntriesByType('navigation')[0].domContentLoadedEventEnd), util: Math.round(window.__util || 0) })));
      await ctx.close();
    }
    return { dcl: mediana(runs.map(r => r.dcl)), util: mediana(runs.map(r => r.util || 99999)), runs };
  };
  const pwa = await medir('/?tela=tarefas');
  // E na mesa, com a abertura: ela so pode durar o que a pagina leva para ficar pronta.
  const mesa = await medir('/index.html', false);
  achado('M4', 'medio', ['splash', 'tarefas'], pwa.util < 2000 && mesa.util < 1000,
    `celular, start_url do PWA: 1a tela util ${pwa.util} ms (mediana de 3: ${pwa.runs.map(r => r.util).join(', ')}); prova < 2000 • mesa com abertura: ${mesa.util} ms (${mesa.runs.map(r => r.util).join(', ')}); prova < 1000`);
  {
    const dir = await medir('/index.html?screen=dashboard');
    achado('M5', 'medio', ['geral'], dir.dcl <= 1600, `celular, Visao geral direta: DCL ${dir.dcl} ms (mediana de 3: ${dir.runs.map(r => r.dcl).join(', ')}); prova <= 1600`);
  }
});

await sonda('M6', async () => {
  const r = {};
  for (const url of ['/index.html?screen=splash', '/index.html?screen=dashboard']) {
    const { ctx, page } = await pagina({ w: 1366, reducedMotion: 'reduce' });
    await page.goto(`${ORIG}${url}`);
    await page.waitForTimeout(900);
    r[url] = await page.evaluate(() => document.getAnimations().filter(a => a.playState === 'running' && a.effect?.getTiming().iterations === Infinity).length);
    await ctx.close();
  }
  achado('M6', 'medio', ['splash', 'topo'], Object.values(r).every(n => n === 0), `animacoes infinitas rodando com prefers-reduced-motion: ${JSON.stringify(r)}`);
});

await sonda('M7', async () => {
  const { ctx, page } = await pagina({ w: 1366 });
  await abrirTela(page, 'tarefas', 600);
  const m = await page.evaluate(() => ['tarefasStatus', 'configStatus'].map(id => { const e = document.getElementById(id); return [id, e.getAttribute('role') || '', e.getAttribute('aria-live') || '']; }));
  await ctx.close();
  achado('M7', 'medio', ['tarefas', 'config'], m.every(([, r, l]) => /status|alert/.test(r) || /polite|assertive/.test(l)), JSON.stringify(m));
});

await sonda('M8', async () => {
  const { ctx, page } = await pagina({ w: 1366 });
  await page.route('**/v1/tasks', async rt => { await new Promise(x => setTimeout(x, 3000)); rt.continue().catch(() => {}); });
  await abrirTela(page, 'tarefas', 500);
  const t = (await page.textContent('#tarefasStatus')).trim();
  await ctx.close();
  achado('M8', 'medio', ['tarefas'], t.length > 0, `carregando: «${t}»`);
});

await sonda('M9', async () => {
  const { ctx, page } = await pagina({ w: 1366 });
  await abrirTela(page, 'config', 1600);
  const m = await page.evaluate(() => {
    const b = [...document.querySelectorAll('#configConteudo .cfg-volta')];
    const cor = b[0] ? getComputedStyle(b[0]).borderTopColor : '';
    return { n: b.length, classes: b[0]?.className ?? '', cor };
  });
  await ctx.close();
  const [r, g, bl] = (m.cor.match(/\d+/g) || [0, 0, 0]).map(Number);
  const rosa = r > 200 && bl > 120 && g < r - 60;
  achado('M9', 'medio', ['config'], m.n > 0 && /\bmarca\b/.test(m.classes) && !/\bexclui\b/.test(m.classes) && rosa, JSON.stringify(m));
});

await sonda('M10', async () => {
  const { ctx, page } = await pagina({ w: 1366 });
  await page.goto(`${ORIG}/index.html`);
  await page.waitForTimeout(200);
  const m = await page.evaluate(() => {
    const cs = s => { const c = getComputedStyle(document.querySelector(s)); return { bg: c.backgroundColor, img: c.backgroundImage }; };
    const opacoNoGradiente = img => [...img.matchAll(/linear-gradient\(([^)]*\([^)]*\))*[^)]*\)/g)].some(m => /rgb\(/.test(m[0]));
    const shell = cs('.app-shell'), splash = cs('.splash');
    return { shell: shell.bg, shellImg: shell.img !== 'none', splash: splash.bg, splashOpaco: opacoNoGradiente(splash.img), tema: document.querySelector('meta[name="theme-color"]').content };
  });
  await ctx.close();
  const man = JSON.parse(readFileSync(join(UI, 'manifest.webmanifest'), 'utf8'));
  const ok = m.shell === 'rgb(1, 4, 24)' && !m.shellImg && m.splash === 'rgb(1, 4, 24)' && !m.splashOpaco && m.tema.toLowerCase() === '#010418' && man.theme_color.toLowerCase() === '#010418';
  achado('M10', 'medio', ['splash', 'topo'], ok, JSON.stringify({ ...m, manifesto: man.theme_color }));
  // STYLE PHOENIX PADRAO (docs/ui/STYLE_PHOENIX_PADRAO.md): os tokens que o PhxClaw adota tem o
  // NOME e o VALOR do console do PhxSql, lidos do fonte de la (nao de uma copia aqui).
  const fontePhx = join(RAIZ, '../phxsql/crates/phxsql-server/ui/index.html');
  const raizPhx = readFileSync(fontePhx, 'utf8').match(/:root\{([\s\S]*?)\n\}/)[1].replace(/\/\*[\s\S]*?\*\//g, '');
  const valorPhx = Object.fromEntries([...raizPhx.matchAll(/(--[a-z0-9-]+)\s*:\s*(#[0-9a-fA-F]{3,8})/g)].map(x => [x[1], x[2].toLowerCase()]));
  const ADOTADOS = ['--fundo', '--painel', '--painel-2', '--painel-3', '--realce', '--linha', '--linha-forte', '--texto', '--texto-2', '--texto-3', '--laranja',
    '--acao-incluir', '--acao-alterar', '--acao-marcar', '--acao-excluir', '--acao-consultar'];
  const { ctx: c2, page: p2 } = await pagina({ w: 1366 });
  await abrirTela(p2, 'agentes', 1200);
  const est = await p2.evaluate(nomes => {
    const cs = getComputedStyle(document.documentElement);
    const tok = Object.fromEntries(nomes.map(n => [n, cs.getPropertyValue(n).trim().toLowerCase()]));
    const tracos = [...new Set([...document.querySelectorAll('.ico svg, .top-actions svg')].map(e => getComputedStyle(e).strokeWidth))];
    return { tok, tracos };
  }, ADOTADOS);
  const fontes = await p2.evaluate(async () => { await document.fonts.ready; return [...document.fonts].filter(f => f.status === 'loaded').map(f => `${f.family.replace(/"/g, '')} ${f.weight}`); });
  await c2.close();
  const divergem = ADOTADOS.filter(n => est.tok[n] !== valorPhx[n]).map(n => `${n}=${est.tok[n]}/${valorPhx[n]}`);
  const temFonte = n => fontes.some(f => f.startsWith(n));
  const okEstilo = !divergem.length && temFonte('Exo 2') && temFonte('IBM Plex Mono') && est.tracos.length === 1;
  achado('M10', 'medio', ['topo'], okEstilo, `Style Phoenix Padrao: tokens divergentes do PhxSql ${divergem.length ? divergem.join(' ') : '0'} de ${ADOTADOS.length}; fontes locais carregadas ${fontes.join(', ')}; espessuras de traco dos icones ${JSON.stringify(est.tracos)}`);
});

await sonda('M11', async () => {
  const { ctx, page } = await pagina({ w: 1366 });
  await abrirTela(page, 'geral', 600);
  const m = await page.evaluate(() => {
    const glifo = /[←-⯿■-◿]/;
    const nav = [...document.querySelectorAll('.nav[data-tela]')].map(b => ({ t: b.dataset.tela, svg: !!b.querySelector('svg'), glifo: glifo.test(b.textContent) }));
    const topo = [...document.querySelectorAll('.topbar button')].map(b => ({ id: b.id, glifo: glifo.test(b.textContent) }));
    return { nav: nav.filter(x => !x.svg || x.glifo).map(x => x.t), topo: topo.filter(x => x.glifo).map(x => x.id || '?') };
  });
  await ctx.close();
  achado('M11', 'medio', ['topo'], !m.nav.length && !m.topo.length, `icones de glifo de fonte: ${JSON.stringify(m)}`);
});

await sonda('M12', async () => {
  const { ctx, page } = await pagina({ w: 1366, lang: 'en' });
  await abrirTela(page, 'tarefas', 1300);
  const datas = await page.$$eval('#tarefasLista td[data-tag="criada"]', tds => tds.map(t => t.textContent.trim()));
  // E o filtro da coluna fala o mesmo formato, sem campo de data nativo (o defeito 4 do
  // Style Phoenix Padrao: a coluna em dd/mm e o filtro pedindo mm/dd do navegador).
  await page.click('#tarefasLista th[data-campo="created_at"] .phx-fbtn');
  await page.waitForTimeout(300);
  const filtro = await page.evaluate(() => { const pop = document.querySelector('.phx-fpop'); return pop ? { nativo: pop.querySelectorAll('input[type=date],input[type=datetime-local]').length, datas: (pop.innerText.match(/\d{1,4}[/-]\d{1,2}[/-]\d{1,4}/g) || []) } : null; });
  await page.keyboard.press('Escape');
  await abrirTela(page, 'geral', 600);
  const hora = await page.$$eval('#eventConsole .event-row small', s => s.map(x => x.textContent));
  await ctx.close();
  const iso = d => /^\d{4}-\d{2}-\d{2}/.test(d);
  achado('M12', 'medio', ['tarefas', 'host'], datas.length > 0 && datas.every(iso) && filtro && !filtro.nativo && filtro.datas.length > 0 && filtro.datas.every(iso),
    `EN: ${datas.slice(0, 2).join(' | ')} • filtro: ${JSON.stringify(filtro).slice(0, 80)} • console: ${hora.slice(0, 1).join('')}`);
});

await sonda('M14', async () => {
  const cat = JSON.parse(readFileSync(join(UI, 'assets/config-catalogo.json'), 'utf8'));
  const semAcento = cat.chaves.filter(c => /\b(nao|configuracao|Binario|binario|padrao|sessao|versao|conexao|so|ate|tambem|politica|diretorio|codigo)\b/.test(`${c.descricao} ${c.motivo_so_ambiente || ''}`)).length;
  const { ctx, page } = await pagina({ w: 1366, lang: 'en', token: false });
  await abrirTela(page, 'config', 1500);
  const d = await page.$$eval('#configConteudo td[data-tag="descricao"]', t => t.slice(0, 40).map(x => x.textContent));
  await ctx.close();
  const pt = new Set(cat.chaves.map(c => c.descricao));
  const emPt = d.filter(x => pt.has(x)).length;
  achado('M14', 'medio', ['config'], semAcento === 0 && emPt === 0, `${semAcento} descricoes do catalogo sem acento; EN: ${emPt}/${d.length} descricoes ainda em PT`);
});

await sonda('M15', async () => {
  const { ctx, page } = await pagina({ w: 390 });
  await abrirTela(page, 'ide', 600);
  await page.click('#ideAbrirBash');
  await page.waitForTimeout(600);
  const m = await page.evaluate(() => {
    const c = document.getElementById('termCanvas').getBoundingClientRect();
    const ped = window.__chamadas.filter(x => x.cmd === 'terminal_abrir' || x.cmd === 'terminal_redimensionar').map(x => x.args.colunas);
    return { largura: Math.round(c.width), colunas: ped };
  });
  await ctx.close();
  const ultimo = m.colunas[m.colunas.length - 1];
  achado('M15', 'medio', ['ide'], ultimo && ultimo * 8.5 <= m.largura + 1, `390: canvas ${m.largura} px, colunas pedidas ${JSON.stringify(m.colunas)}`, { so390: true });
});

await sonda('M16', async () => {
  const { ctx, page } = await pagina({ w: 390 });
  const r = {};
  for (const t of ['tarefas', 'config']) {
    await abrirTela(page, t, t === 'config' ? 1500 : 900);
    r[t] = await page.evaluate(() => {
      const sec = document.querySelector('section.tela:not([hidden])');
      return [...sec.querySelectorAll('button,a[href],input,select,textarea')].filter(e => e.getClientRects().length && getComputedStyle(e).visibility !== 'hidden')
        .filter(e => { const b = e.getBoundingClientRect(); return b.width < 24 || b.height < 24; }).length;
    });
  }
  await ctx.close();
  achado('M16', 'medio', ['tarefas', 'config'], Object.values(r).every(n => n === 0), `alvos de toque < 24 px a 390: ${JSON.stringify(r)}`, { so390: true });
});

await sonda('C1|C4|C5', async () => {
  const { ctx, page } = await pagina({ w: 1366 });
  await abrirTela(page, 'geral', 600);
  await page.focus('.hero-actions .acao');
  await page.keyboard.press('Tab');
  const m = await page.evaluate(() => { const c = getComputedStyle(document.activeElement); return `${c.outlineStyle} ${c.outlineWidth} ${c.outlineColor}`; });
  const rodape = await page.evaluate(() => !!document.querySelector('.footerbar')?.closest('main'));
  const avatar = await page.evaluate(() => document.querySelector('.avatar')?.textContent ?? null);
  await ctx.close();
  // O anel e o --laranja do Style Phoenix Padrao (o PhxSql usa o mesmo :focus-visible).
  achado('C1', 'cosmetico', ['topo'], m === 'solid 2px rgb(255, 138, 28)', `anel de foco de uma acao: ${m}`);
  achado('C4', 'cosmetico', ['topo'], avatar === null, `avatar fixo: ${avatar}`);
  achado('C5', 'cosmetico', ['rodape'], !rodape, `rodape dentro de <main>: ${rodape}`);
});

await sonda('C2', async () => {
  const { ctx, page } = await pagina({ w: 1366 });
  await abrirTela(page, 'geral', 300);
  const soltas = await page.evaluate(() => {
    const r = [];
    for (const ss of document.styleSheets) {
      if (!/assets\/(app|tarefas|grades)\.css/.test(ss.href || '')) continue;
      for (const rule of ss.cssRules) if (rule.selectorText && /\.acao[^,]*:hover/.test(rule.selectorText)) r.push(rule.selectorText.slice(0, 40));
    }
    return r;
  });
  await ctx.close();
  achado('C2', 'cosmetico', ['topo'], soltas.length === 0, `hover das acoes fora de @media(hover:hover): ${soltas.length}`);
});

await browser.close();
srv.close();

/* ============================ VEREDITO ============================ */
const VER = {};
for (const [k, t] of Object.entries(QTELAS)) {
  const meus = achados.filter(a => a.telas.includes(k));
  const falhos = meus.filter(a => !a.ok);
  const fortes = falhos.filter(a => a.sev === 'bloqueia' || a.sev === 'grave');
  const nao = fortes.filter(a => a.principal && !(a.so390 && t.mesa));
  const veredito = nao.length ? 'NAO QUALIFICADA' : fortes.length ? 'QUALIFICADA COM RESSALVAS' : 'QUALIFICADA';
  VER[k] = { nome: t.nome, veredito, fortes: [...new Set(fortes.map(a => a.id))], outros: [...new Set(falhos.filter(a => !fortes.includes(a)).map(a => a.id))] };
}
if (SO) {
  console.log(`sondas pedidas: ${achados.filter(a => a.ok).length}/${achados.length} ok`);
  writeFileSync(join(OUT, 'qualificar-parcial.json'), JSON.stringify({ ui: UI, achados }, null, 1));
  process.exit(achados.some(a => !a.ok) ? 1 : 0);
}
console.log('\nVEREDITO POR TELA (criterio do relatorio de 01/10/2026)');
for (const v of Object.values(VER)) console.log(`  ${v.nome.padEnd(26)} ${v.veredito.padEnd(26)} ${v.fortes.length ? `[${v.fortes.join(' ')}]` : ''}${v.outros.length ? ` medios/cosmeticos: ${v.outros.join(' ')}` : ''}`);
const nFalhas = achados.filter(a => !a.ok).length;
console.log(`sondas: ${achados.length - nFalhas}/${achados.length} ok • telas QUALIFICADAS: ${Object.values(VER).filter(v => v.veredito === 'QUALIFICADA').length}/${Object.keys(VER).length}`);
writeFileSync(join(OUT, SO ? 'qualificar-parcial.json' : 'qualificar.json'), JSON.stringify({ ui: UI, achados, veredito: VER }, null, 1));
if (SO) process.exit(achados.some(a => !a.ok) ? 1 : 0);
process.exit(Object.values(VER).every(v => v.veredito === 'QUALIFICADA') ? 0 : 1);
