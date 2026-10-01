/* Prova da linha "Verificar estoque" sobre o mock renderizado (sem a loja):
 *   - o link aponta para o WhatsApp configurado, com produto e variante na
 *     mensagem, e abre em aba nova;
 *   - trocar a variante troca a mensagem (e so ela);
 *   - a linha fica abaixo do botao de comprar, dentro do cartao, a 390 e 1280 px;
 *   - sem numero configurado, volta "Em estoque" e nao sobra link morto.
 *
 * Uso:
 *   node mock/render.mjs sections/product-whatsapp.liquid bambu-lab-a1 /tmp/com.html '{"Padrão":417,"Combo":2580}'
 *   AJUSTES='{"whatsapp_number":""}' node mock/render.mjs sections/product-whatsapp.liquid bambu-lab-a1 /tmp/sem.html '{"Padrão":417,"Combo":2580}'
 *   node mock/prova-estoque.mjs /tmp/com.html /tmp/sem.html [pasta-de-capturas]
 * Sai com codigo 1 se alguma verificacao falhar.
 */
import { chromium } from '/opt/node22/lib/node_modules/playwright/index.mjs';
import { mkdirSync } from 'node:fs';
import { resolve } from 'node:path';

const [,, comNumero, semNumero, saida = '.'] = process.argv;
if (!comNumero || !semNumero) { console.error('uso: prova-estoque.mjs <com-numero.html> <sem-numero.html> [pasta]'); process.exit(2); }
mkdirSync(saida, { recursive: true });

const NUMERO = '5541995712681';
const falhas = [];
const confere = (ok, msg) => { console.log((ok ? 'ok    ' : 'FALHA ') + msg); if (!ok) falhas.push(msg); };
const browser = await chromium.launch();

for (const [w, h] of [[390, 844], [1280, 900]]) {
  const page = await (await browser.newContext({ viewport: { width: w, height: h } })).newPage();
  await page.goto('file://' + resolve(comNumero), { waitUntil: 'load' });

  const le = () => page.evaluate(() => {
    const a = document.querySelector('[data-pp-wa]');
    const btn = document.querySelector('[data-pp-add]');
    const card = document.querySelector('.pp__card');
    const r = a && a.getBoundingClientRect(), b = btn.getBoundingClientRect(), c = card.getBoundingClientRect();
    return a ? {
      texto: a.innerText.trim(), href: a.href, alvo: a.target, rel: a.rel,
      abaixoDoBotao: r.top >= b.bottom, dentroDoCartao: r.left >= c.left && r.right <= c.right,
      centro: Math.abs((r.left + r.right) / 2 - (b.left + b.right) / 2),
      cor: getComputedStyle(a).color, sublinhado: getComputedStyle(a).textDecorationLine,
    } : null;
  });
  let m = await le();
  confere(!!m && m.texto === 'Verificar estoque', `${w}px: link "Verificar estoque" existe (${m && m.texto})`);
  if (!m) { continue; }
  const msg = decodeURIComponent(m.href.split('text=')[1] || '').replace(/\+/g, ' ');
  confere(m.href.startsWith('https://wa.me/' + NUMERO + '?text='), `${w}px: aponta para o WhatsApp ${NUMERO}`);
  confere(msg === 'Olá! Quero verificar o estoque de Bambu Lab A1 (Padrão).', `${w}px: mensagem com produto e variante ("${msg}")`);
  confere(m.alvo === '_blank' && /noopener/.test(m.rel), `${w}px: abre em aba nova sem vazar a janela (target ${m.alvo}, rel ${m.rel})`);
  confere(m.abaixoDoBotao && m.dentroDoCartao, `${w}px: abaixo do botao e dentro do cartao`);
  confere(m.centro <= 2, `${w}px: centrado sob o botao (desvio ${m.centro.toFixed(1)} px)`);
  confere(m.sublinhado.includes('underline'), `${w}px: sublinhado, para parecer link (${m.sublinhado})`);

  // troca de variante: a mensagem segue a escolha, o resto do link fica
  const combo = page.locator('[data-pp-radio][value="Combo"]');
  if (await combo.count()) await combo.first().dispatchEvent('change', {}, { force: true }).catch(() => {});
  if (await combo.count()) await page.evaluate(() => { const r = document.querySelector('[data-pp-radio][value="Combo"]'); r.checked = true; r.dispatchEvent(new Event('change', { bubbles: true })); });
  else await page.selectOption('[data-pp-select]', 'Combo');
  await page.waitForTimeout(150);
  const m2 = await le();
  const msg2 = decodeURIComponent(m2.href.split('text=')[1] || '').replace(/\+/g, ' ');
  confere(msg2 === 'Olá! Quero verificar o estoque de Bambu Lab A1 (Combo).', `${w}px: ao escolher Combo a mensagem muda ("${msg2}")`);
  confere(m2.href.startsWith('https://wa.me/' + NUMERO + '?text='), `${w}px: numero continua o mesmo depois da troca`);

  // a linha fica abaixo da dobra; rola ate ela antes de recortar, porque o
  // recorte e em coordenadas da janela e nao da pagina
  await page.locator('[data-pp-stock]').scrollIntoViewIfNeeded();
  const r = await page.locator('[data-pp-stock]').boundingBox();
  const topo = Math.max(0, r.y - 90);
  await page.screenshot({ path: `${saida}/estoque-${w}.png`, clip: { x: 0, y: topo, width: w, height: Math.min(h - topo, r.height + 130) } });

  // sem numero: volta o status pela contagem, e nenhum link morto
  await page.goto('file://' + resolve(semNumero), { waitUntil: 'load' });
  const s = await page.evaluate(() => ({ link: !!document.querySelector('[data-pp-wa]'), texto: document.querySelector('[data-pp-stock]').innerText.trim() }));
  confere(!s.link && s.texto === 'Em estoque', `${w}px sem numero: volta "${s.texto}" sem link`);
  await page.context().close();
}

await browser.close();
console.log(falhas.length ? `\n${falhas.length} falha(s)` : '\ntudo certo');
process.exit(falhas.length ? 1 : 0);
