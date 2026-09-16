import { chromium } from '/opt/node22/lib/node_modules/playwright/index.mjs';
import { resolve } from 'node:path';
const here = resolve('mock');
const b = await chromium.launch();
const out = {};
const css = (p, sel, prop) => p.evaluate(([s, pr]) => { const e = document.querySelector(s); return e ? getComputedStyle(e)[pr] : null; }, [sel, prop]);
for (const nome of ['a1', 'h2s', 'ams']) {
  const url = 'file://' + resolve(here, nome + '.html');
  const r = (out[nome] = {});
  // ---------- celular ----------
  const m = await b.newContext({ viewport: { width: 412, height: 915 }, deviceScaleFactor: 2, isMobile: true, hasTouch: true, locale: 'pt-BR' });
  const p = await m.newPage(); const erros = []; p.on('pageerror', (e) => erros.push(String(e))); p.on('console', (c) => { if (c.type() === 'error') erros.push(c.text()); });
  await p.goto(url); await p.waitForTimeout(700);
  await p.screenshot({ path: `mock/shot-${nome}-mob-top.png` });
  await p.screenshot({ path: `mock/shot-${nome}-mob-full.png`, fullPage: true });
  r.slides = await p.locator('.pp__slide').count();
  r.thumbs = await p.locator('.pp__thumb').count();
  r.cards = await p.locator('.pp__vcard').evaluateAll((els) => els.map((e) => e.textContent.replace(/\s+/g, ' ').trim()));
  r.stock = (await p.locator('[data-pp-stock]').textContent()).trim();
  r.trust = await p.locator('.pp__trust li').count();
  r.discs = await p.locator('.pp__disc summary span').evaluateAll((els) => els.map((e) => e.textContent.trim()));
  r.stageBg = await css(p, '.pp__stage', 'backgroundColor');
  r.stagePad = await css(p, '.pp__stage', 'padding');
  r.stageBorder = await css(p, '.pp__stage', 'borderWidth');
  r.thumbsWrap = await css(p, '.pp__thumbs', 'flexWrap');
  if (r.slides > 1) {
    // crossfade: clica na 3a miniatura e mede a opacidade NO MEIO da transicao
    await p.locator('[data-pp-thumb="2"]').click();
    await p.waitForTimeout(110);
    r.meio = await p.evaluate(() => [...document.querySelectorAll('.pp__slide')].slice(0, 3).map((s) => +getComputedStyle(s).opacity));
    await p.screenshot({ path: `mock/shot-${nome}-mob-meio.png`, clip: { x: 0, y: 64, width: 412, height: 420 } });
    await p.waitForTimeout(400);
    r.aposClique = { contador: (await p.locator('[data-pp-count]').textContent()).trim(), ativo: await p.locator('.pp__slide.is-active').getAttribute('data-pp-slide'), thumbAtiva: await p.locator('.pp__thumb.is-active').getAttribute('data-pp-thumb') };
    // swipe para a esquerda -> proxima
    const box = await p.locator('.pp__stage').boundingBox();
    await p.mouse.move(box.x + box.width * 0.8, box.y + box.height / 2); await p.mouse.down();
    await p.mouse.move(box.x + box.width * 0.2, box.y + box.height / 2, { steps: 6 }); await p.mouse.up();
    await p.waitForTimeout(400);
    r.aposSwipe = (await p.locator('[data-pp-count]').textContent()).trim();
    // zoom: toque simples no palco
    await p.mouse.click(box.x + box.width / 2, box.y + box.height / 2); await p.waitForTimeout(300);
    r.zoomAberto = await p.evaluate(() => { const d = document.querySelector('[data-pp-lightbox]'); return d ? d.open : null; });
    await p.screenshot({ path: `mock/shot-${nome}-mob-zoom.png` });
    await p.keyboard.press('Escape'); await p.waitForTimeout(200);
    r.zoomFechado = await p.evaluate(() => !document.querySelector('[data-pp-lightbox]').open);
  }
  // barra fixa: rola ate o botao sair por cima
  await p.evaluate(() => window.scrollTo(0, document.body.scrollHeight * 0.6)); await p.waitForTimeout(500);
  r.stickyOn = await p.evaluate(() => document.querySelector('[data-pp-sticky]')?.classList.contains('is-on'));
  r.stickyPreco = (await p.locator('[data-pp-sticky-price]').textContent().catch(() => '')).trim();
  await p.screenshot({ path: `mock/shot-${nome}-mob-sticky.png` });
  // troca de variante pelo cartao
  if (r.cards.length > 1) {
    await p.evaluate(() => window.scrollTo(0, 0)); await p.waitForTimeout(200);
    const preco0 = (await p.locator('[data-pp-pix]').textContent()).trim();
    await p.locator('.pp__vcard').nth(1).click(); await p.waitForTimeout(300);
    r.variante = { antes: preco0, depois: (await p.locator('[data-pp-pix]').textContent()).trim(), id: await p.locator('[data-pp-id]').inputValue(), stock: (await p.locator('[data-pp-stock]').textContent()).trim(), sticky: (await p.locator('[data-pp-sticky-price]').textContent()).trim(), contador: (await p.locator('[data-pp-count]').textContent().catch(() => '')).trim() };
    await p.screenshot({ path: `mock/shot-${nome}-mob-variante.png` });
  }
  r.errosMobile = erros;
  await m.close();
  // ---------- desktop ----------
  const d = await b.newContext({ viewport: { width: 1440, height: 900 }, locale: 'pt-BR' });
  const q = await d.newPage(); const erros2 = []; q.on('pageerror', (e) => erros2.push(String(e)));
  await q.goto(url); await q.waitForTimeout(700);
  await q.screenshot({ path: `mock/shot-${nome}-desk-top.png` });
  if (r.slides > 1) {
    r.setaAntesHover = await css(q, '.pp__nav--next', 'opacity');
    await q.locator('.pp__stage').hover(); await q.waitForTimeout(250);
    r.setaNoHover = await css(q, '.pp__nav--next', 'opacity');
    await q.locator('[data-pp-next]').click(); await q.waitForTimeout(400);
    r.deskContador = (await q.locator('[data-pp-count]').textContent()).trim();
    await q.locator('.pp__stage').focus(); await q.keyboard.press('ArrowRight'); await q.waitForTimeout(400);
    r.deskTeclado = (await q.locator('[data-pp-count]').textContent()).trim();
    await q.screenshot({ path: `mock/shot-${nome}-desk-hover.png` });
  }
  r.deskStickyVisivel = await css(q, '.pp__sticky', 'display');
  r.errosDesktop = erros2;
  await d.close();
}
await b.close();
console.log(JSON.stringify(out, null, 1));
