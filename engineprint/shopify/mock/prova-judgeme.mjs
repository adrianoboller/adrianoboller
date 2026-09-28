/* Prova do Judge.me no preview de um tema: o widget carrega? as estrelas
 * ficam ao lado do nome? a pagina da loja aparece em portugues?
 *
 * Duas passadas por largura:
 *   real      — a loja como esta (hoje sem avaliacao): o widget tem de
 *               montar e o selo tem de sumir (hide_badge_preview_if_no_reviews).
 *   simulada  — o selo do produto recebe 2 avaliacoes, nota 4,5, no formato
 *               do molde do proprio app (shopify_v2/badge), antes do app
 *               arruma-lo — mede estrela ao lado do nome sem gravar
 *               avaliacao falsa na loja.
 *
 * Erro de console so reprova se for NOVO: a mesma pagina e aberta no tema
 * de base (o publicado) e o que ja acontece la nao e deste tema. Requisicao
 * abortada (net::ERR_ABORTED) nao conta: e o teste fechando a pagina com
 * coisa em voo, e muda de corrida para corrida.
 *
 * Uso:  node mock/prova-judgeme.mjs <id-do-tema> <id-do-tema-base> [handle] [pasta-de-capturas]
 *       SO=produto|simulado|loja limita a uma parte (a base e medida inteira mesmo assim)
 * Sai com codigo 1 se alguma verificacao falhar.
 */
import { chromium } from '/opt/node22/lib/node_modules/playwright/index.mjs';
import { mkdirSync } from 'node:fs';

const LOJA = 'https://engineprint.com.br';
const tema = process.argv[2];
const base = process.argv[3];
const handle = process.argv[4] || 'bambu-lab-a1';
const saida = process.argv[5] || '.';
if (!tema || !base) { console.error('uso: prova-judgeme.mjs <id-do-tema> <id-do-tema-base> [handle] [pasta]'); process.exit(2); }
mkdirSync(saida, { recursive: true });

const LARGURAS = [[390, 844], [1280, 900]];
const SELO_SIMULADO =
  "<div class='jdgm-prev-badge' data-average-rating='4.50' data-number-of-reviews='2' data-number-of-questions='0'>" +
  "<span class='jdgm-prev-badge__stars' data-score='4.50' tabindex='0' aria-label='4.50 stars' role='button'>" +
  "<span class='jdgm-star jdgm--on'></span><span class='jdgm-star jdgm--on'></span><span class='jdgm-star jdgm--on'></span>" +
  "<span class='jdgm-star jdgm--on'></span><span class='jdgm-star jdgm--half'></span></span>" +
  "<span class='jdgm-prev-badge__text'> 2 reviews </span></div>";

// tira o que muda a cada carga (parametros, ids de rastreio) para comparar
// erro com erro entre os dois temas
const assinatura = (s) => s.replace(/\?[^\s)|]*/g, '').replace(/[0-9a-f]{8}-[0-9a-f-]{27,}/gi, '<id>').slice(0, 150);
/* Telemetria do proprio Shopify (otlp/monorail em shopifysvc.com, e o
   event_observer_reporter que posta nela) falha as vezes por CORS ou
   "Failed to fetch" sem relacao com o tema: numa corrida longa ela reprovou
   o candidato, e depois, medido, deu 0 em 4 cargas no tema publicado e 0 em
   4 no candidato (28/09/2026). Nao e codigo do tema; fica fora da conta. */
const TELEMETRIA = /shopifysvc\.com|event_observer_reporter/;
const relevantes = (lista) => new Set(lista.filter((e) => !/ERR_ABORTED/.test(e) && !TELEMETRIA.test(e)).map(assinatura));

const SO = process.env.SO || '';
const roda = (parte) => !SO || SO === parte;

const falhas = [];
const confere = (ok, msg) => { console.log((ok ? 'ok    ' : 'FALHA ') + msg); if (!ok) falhas.push(msg); };

const browser = await chromium.launch({ proxy: { server: process.env.HTTPS_PROXY } });

async function abre(w, h, simular, temaId = tema) {
  const ctx = await browser.newContext({ viewport: { width: w, height: h }, deviceScaleFactor: 1 });
  const page = await ctx.newPage();
  const erros = [];
  page.on('pageerror', (e) => erros.push(String(e.message).slice(0, 160)));
  page.on('console', (m) => { if (m.type() === 'error') erros.push(m.text().slice(0, 160)); });
  // registra de onde veio cada falha de recurso: o texto do console nao traz a URL
  page.on('requestfailed', (r) => erros.push(`falhou ${r.url().slice(0, 110)} (${r.failure() && r.failure().errorText})`));
  page.on('response', (r) => { if (r.status() >= 400) erros.push(`${r.status()} ${r.url().slice(0, 110)}`); });
  if (simular) {
    /* O selo e preenchido no proprio navegador, enquanto o HTML e lido e
       antes do docReady em que o app arruma os selos. A primeira versao
       buscava o HTML pelo Node (route.fetch) e o entregava trocado: era um
       segundo cliente na mesma sessao, e o Shopify respondeu 503 ao pedido
       de secao do cabecalho em 2 de 3 corridas a 1280 px — contra 0 em 6
       cargas normais em cada tema. O erro era do metodo, nao do tema. */
    await ctx.addInitScript((selo) => {
      const obs = new MutationObserver(() => {
        const el = document.querySelector('.pp__estrelas');
        if (el && !el.dataset.simulado) { el.innerHTML = selo; el.dataset.simulado = '1'; obs.disconnect(); }
      });
      obs.observe(document, { childList: true, subtree: true });
    }, SELO_SIMULADO);
  }
  // o cookie de preview sai desta visita; sem ele a loja serve o tema publicado
  try {
    await page.goto(`${LOJA}/?preview_theme_id=${temaId}`, { waitUntil: 'domcontentloaded' });
  } catch (e) {
    if (/ERR_CERT_AUTHORITY_INVALID/.test(e.message)) {
      console.error('O Chromium nao confia no CA do proxy: ~/.pki/nssdb esta sem ele (medido vazio em 28/09/2026).\n' +
        'Rode  bash mock/confia-ca-do-proxy.sh  e tente de novo — o porque esta em\n' +
        'cognicao/cognicao_navegador_sem_ca_do_proxy_20260928_2000.md. Nao desligue a verificacao TLS.');
      process.exit(2);
    }
    throw e;
  }
  return { ctx, page, erros };
}

async function temaServido(page) {
  return page.evaluate(() => (window.Shopify && window.Shopify.theme && String(window.Shopify.theme.id)) || '');
}

async function errosDaBase(caminho, w, h, espera) {
  const { ctx, page, erros } = await abre(w, h, false, base);
  await page.goto(`${LOJA}${caminho}`, { waitUntil: 'load' });
  await page.waitForTimeout(espera);
  const servido = await temaServido(page);
  await ctx.close();
  if (servido !== base) falhas.push(`base: ${caminho} servido pelo tema ${servido}, nao ${base}`);
  return relevantes(erros);
}

function confereErrosNovos(rotulo, erros, conhecidos) {
  const novos = [...relevantes(erros)].filter((e) => !conhecidos.has(e));
  confere(novos.length === 0, `${rotulo}: nenhum erro de console novo em relacao ao tema ${base}${novos.length ? ' — ' + novos.join(' | ') : ''}`);
}

/* A base e a UNIAO do que o tema publicado produz em todas as paginas e
   larguras testadas: os erros dos pixels do Shopify (web-pixels, sandbox
   bloqueado por resposta) aparecem ou nao conforme o tempo da carga, e a
   comparacao pagina a pagina reprovou o tema candidato por um erro que a
   base tambem da — so nao deu naquela corrida (medido: 7 na base numa, 5
   na seguinte). */
const daBase = new Set();
for (const [w, h] of LARGURAS) {
  for (const [caminho, espera] of [[`/products/${handle}`, 8000], ['/pages/avaliacoes', 8000]]) {
    for (const e of await errosDaBase(caminho, w, h, espera)) daBase.add(e);
  }
}
console.log(`base ${base}: ${daBase.size} erro(s) de console distintos nas paginas testadas`);
const baseProduto = daBase, baseLoja = daBase;

for (const [w, h] of LARGURAS) {

  // ---------- produto, real ----------
  if (roda('produto')) {
    const { ctx, page, erros } = await abre(w, h, false);
    await page.goto(`${LOJA}/products/${handle}`, { waitUntil: 'load' });
    confere((await temaServido(page)) === tema, `${w}px produto: servido pelo tema ${tema}`);
    const t0 = Date.now();
    const montou = await page.waitForFunction(() => {
      const el = document.getElementById('judgeme_product_reviews');
      return el && el.innerText.trim().length > 0;
    }, null, { timeout: 20000 }).then(() => true).catch(() => false);
    const widget = await page.evaluate(() => {
      const el = document.getElementById('judgeme_product_reviews');
      const r = el.getBoundingClientRect();
      return { texto: el.innerText.trim().replace(/\s+/g, ' ').slice(0, 160), classes: el.className, larg: Math.round(r.width) };
    });
    confere(montou, `${w}px produto: widget montou ("${widget.texto}")`);
    const selo = await page.evaluate(() => {
      const el = document.querySelector('.pp__estrelas');
      return el ? { display: getComputedStyle(el).display, alt: Math.round(el.getBoundingClientRect().height) } : null;
    });
    confere(selo && (selo.display === 'none' || selo.alt === 0), `${w}px produto sem avaliacao: selo escondido (display ${selo && selo.display}, altura ${selo && selo.alt})`);
    await page.locator('#avaliacoes').scrollIntoViewIfNeeded();
    await page.waitForTimeout(Math.max(600, 6000 - (Date.now() - t0)));
    await page.locator('#avaliacoes').screenshot({ path: `${saida}/produto-widget-${w}.png` });
    confereErrosNovos(`${w}px produto`, erros, baseProduto);
    await ctx.close();
  }

  // ---------- produto, simulado (2 avaliacoes, 4,5) ----------
  if (roda('simulado')) {
    const { ctx, page, erros } = await abre(w, h, true);
    await page.goto(`${LOJA}/products/${handle}`, { waitUntil: 'load' });
    if (!(await page.evaluate(() => !!document.querySelector('.pp__estrelas[data-simulado]')))) falhas.push('simulacao: o selo nao foi preenchido');
    const pronto = await page.waitForFunction(() => {
      const el = document.querySelector('.pp__estrelas');
      const txt = el && el.querySelector('.jdgm-prev-badge__text');
      // textContent e nao innerText: no celular o texto fica oculto de proposito
      return el && el.getBoundingClientRect().height > 0 && txt && /avalia/i.test(txt.textContent);
    }, null, { timeout: 20000 }).then(() => true).catch(() => false);
    const m = await page.evaluate(() => {
      const t = document.querySelector('.pp__title').getBoundingClientRect();
      const e = document.querySelector('.pp__estrelas').getBoundingClientRect();
      const c = document.querySelector('.pp__card').getBoundingClientRect();
      const estrela = document.querySelector('.pp__estrelas .jdgm-star');
      return {
        titulo: { x: t.x, y: t.y, r: t.right, b: t.bottom, h: t.height },
        selo: { x: e.x, y: e.y, r: e.right, b: e.bottom, h: e.height },
        cartao: { x: c.x, r: c.right },
        cor: estrela ? getComputedStyle(estrela).color : null,
        // o app forca display:block no selo, entao gap nao vale; a folga e medida
        folga: (() => {
          const st = document.querySelector('.pp__estrelas .jdgm-prev-badge__stars').getBoundingClientRect();
          const tx = document.querySelector('.pp__estrelas .jdgm-prev-badge__text').getBoundingClientRect();
          return tx.width > 0 ? tx.left - st.right : null;
        })(),
        texto: document.querySelector('.pp__estrelas .jdgm-prev-badge__text').textContent.trim(),
        visivel: document.querySelector('.pp__estrelas').innerText.trim().replace(/\s+/g, ' ') +
          getComputedStyle(document.querySelector('.pp__estrelas .jdgm-prev-badge'), '::after').content.replace(/^none$|"/g, ''),
      };
    });
    confere(pronto, `${w}px simulado: selo apareceu com texto em portugues ("${m.texto}"; na tela: "${m.visivel}")`);
    // "ao lado": selo comeca a direita do fim do titulo, na mesma faixa vertical;
    // ou, se o nome nao couber, logo abaixo — nunca por cima do titulo
    const mesmaLinha = m.selo.x >= m.titulo.r - 1 && m.selo.y < m.titulo.b && m.selo.b > m.titulo.y;
    const abaixo = m.selo.y >= m.titulo.b - 1;
    confere(mesmaLinha || abaixo, `${w}px simulado: selo ${mesmaLinha ? 'ao lado do nome' : abaixo ? 'abaixo do nome' : 'SOBREPOSTO ao nome'} (titulo x ${Math.round(m.titulo.x)}–${Math.round(m.titulo.r)}, selo x ${Math.round(m.selo.x)}–${Math.round(m.selo.r)})`);
    confere(m.selo.r <= m.cartao.r + 0.5, `${w}px simulado: selo dentro do cartao (direita ${Math.round(m.selo.r)} <= ${Math.round(m.cartao.r)})`);
    if (m.folga !== null) confere(m.folga >= 4, `${w}px simulado: folga entre estrelas e texto (${m.folga.toFixed(1)} px >= 4)`);
    confere(m.cor === 'rgb(17, 17, 17)', `${w}px simulado: estrela na cor das estrelas da loja (${m.cor})`);
    const regiao = await page.locator('.pp__head').boundingBox();
    await page.screenshot({ path: `${saida}/produto-estrelas-${w}.png`, clip: { x: 0, y: Math.max(0, regiao.y - 20), width: w, height: Math.min(260, regiao.height + 40) } });
    confereErrosNovos(`${w}px simulado`, erros, baseProduto);
    await ctx.close();
  }

  // ---------- pagina da loja ----------
  if (roda('loja')) {
    const { ctx, page, erros } = await abre(w, h, false);
    await page.goto(`${LOJA}/pages/avaliacoes`, { waitUntil: 'load' });
    confere((await temaServido(page)) === tema, `${w}px pagina da loja: servida pelo tema ${tema}`);
    await page.waitForTimeout(4000);
    const p = await page.evaluate(() => {
      const s = document.querySelector('.wxav');
      const vis = (sel) => { const el = document.querySelector(sel); if (!el) return null; const r = el.getBoundingClientRect(); return r.width > 0 && r.height > 0 && getComputedStyle(el).visibility !== 'hidden'; };
      return {
        texto: s ? s.innerText.replace(/\s+/g, ' ').trim() : '',
        placar: vis('.wxav__placar'),
        resumoIngles: vis('.jdgm-all-reviews__summary'),
        botao: (() => { const el = document.querySelector('.wxav .jdgm-write-rev-link'); return el ? getComputedStyle(el).backgroundColor : null; })(),
        estrela: (() => { const el = document.querySelector('.wxav .jdgm-histogram .jdgm-star'); return el ? getComputedStyle(el).color : null; })(),
        zerada: !!document.querySelector(".wxav .jdgm-all-reviews__header[data-number-of-reviews='0']"),
        girando: vis('.wxav .jdgm-spinner'),
        botaoX: (() => { const el = document.querySelector('.wxav .jdgm-write-rev-link'); return el ? Math.round(el.getBoundingClientRect().x) : null; })(),
        placarX: Math.round(document.querySelector('.wxav__placar').getBoundingClientRect().x),
        fio: (() => { const el = document.querySelector('.wxav .jdgm-widget-actions-wrapper'); return el ? getComputedStyle(el).borderLeftWidth : null; })(),
        larg: s ? Math.round(s.scrollWidth) : 0,
        docLarg: document.documentElement.scrollWidth,
      };
    });
    confere(p.placar === true, `${w}px pagina da loja: placar visivel ("${p.texto.slice(0, 120)}")`);
    confere(!/\b(Be the first|Write a review|reviews?\b)/.test(p.texto), `${w}px pagina da loja: nenhum texto em ingles visivel`);
    confere(p.docLarg <= w, `${w}px pagina da loja: sem rolagem lateral (${p.docLarg} <= ${w})`);
    confere(p.botao === 'rgb(10, 10, 10)', `${w}px pagina da loja: botao de avaliar na cor da loja (${p.botao})`);
    if (p.estrela) confere(p.estrela === 'rgb(17, 17, 17)', `${w}px pagina da loja: estrela do app na cor da loja (${p.estrela})`);
    if (p.zerada) confere(!p.girando, `${w}px pagina da loja sem avaliacao: nenhum carregador girando a toa`);
    if (p.zerada) confere(p.botaoX === p.placarX && p.fio === '0px', `${w}px pagina da loja sem avaliacao: botao alinhado ao placar e sem fio solto (x ${p.botaoX} vs ${p.placarX}, fio ${p.fio})`);
    await page.screenshot({ path: `${saida}/pagina-loja-${w}.png`, fullPage: false });
    confereErrosNovos(`${w}px pagina da loja`, erros, baseLoja);
    await ctx.close();
  }
}

await browser.close();
console.log(falhas.length ? `\n${falhas.length} falha(s)` : '\ntudo certo');
process.exit(falhas.length ? 1 : 0);
