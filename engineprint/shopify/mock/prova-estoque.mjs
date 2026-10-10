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

/* Modo loja: --tema <id> <url-do-produto> [pasta]. Abre o preview daquele
   tema (cookie) e faz as mesmas conferencias no HTML que a Shopify serve.
   O caminho "sem numero" nao se prova ao vivo: exigiria mudar a
   configuracao do tema. */
const args = process.argv.slice(2);
let tema = null;
if (args[0] === '--tema') { tema = args[1]; args.splice(0, 2); }
// no modo loja nao ha segundo HTML: o que vem depois da URL e a pasta
const [comNumero, semNumero, saida = '.'] = tema ? [args[0], null, args[1] || '.'] : args;
if (!comNumero || (!tema && !semNumero)) {
  console.error('uso: prova-estoque.mjs <com-numero.html> <sem-numero.html> [pasta]\n     prova-estoque.mjs --tema <id> <url-do-produto> [pasta]');
  process.exit(2);
}
mkdirSync(saida, { recursive: true });
const alvo = (p) => (tema ? p : 'file://' + resolve(p));

const NUMERO = '5541995712681';
const falhas = [];
const confere = (ok, msg) => { console.log((ok ? 'ok    ' : 'FALHA ') + msg); if (!ok) falhas.push(msg); };
const browser = await chromium.launch(tema ? { proxy: { server: process.env.HTTPS_PROXY } } : {});

for (const [w, h] of [[390, 844], [1280, 900]]) {
  const page = await (await browser.newContext({ viewport: { width: w, height: h } })).newPage();
  // o cookie de preview sai desta visita; sem ele a loja serve o tema publicado
  if (tema) await page.goto(new URL('/?preview_theme_id=' + tema, comNumero).href, { waitUntil: 'domcontentloaded' });
  await page.goto(alvo(comNumero), { waitUntil: 'load' });
  if (tema) {
    const servido = await page.evaluate(() => (window.Shopify && window.Shopify.theme && String(window.Shopify.theme.id)) || '');
    confere(servido === tema, `${w}px: servido pelo tema ${tema} (${servido})`);
  }

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
      bolinha: (() => { const d = a.querySelector('.pp__dot'); return d ? getComputedStyle(d).backgroundColor : null; })(),
      pulso: (() => { const d = a.querySelector('.pp__dot'); return d ? getComputedStyle(d).animationName + ' ' + getComputedStyle(d).animationDuration : null; })(),
      precoTexto: (document.querySelector('.pp__pricebox') || { innerText: '' }).innerText.replace(/\s+/g, ' ').trim(),
      secaoTexto: (document.querySelector('.pp') || { innerText: '' }).innerText,
      icone: !!a.querySelector('svg'),
      transicao: getComputedStyle(a).transitionProperty + ' ' + getComputedStyle(a).transitionDuration + ' ' + getComputedStyle(a).transitionTimingFunction,
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
  confere(m.bolinha === 'rgb(29, 122, 58)' && !m.icone, `${w}px: a bolinha verde de antes, sem icone (${m.bolinha}, icone ${m.icone})`);
  confere(/^pp-pulso 2s$/.test(m.pulso || ''), `${w}px: a bolinha verde pulsa (${m.pulso})`);
  // o Pix saiu (10/10/2026): nem desconto nem "no Pix" em lugar nenhum da secao
  const sobras = (m.secaoTexto.match(/no Pix|% de desconto/gi) || []).length;
  confere(sobras === 0, `${w}px: nenhuma mencao a desconto no Pix na secao (${sobras})`);
  confere(/^R\$ [\d.]+,\d\d ou 2x de R\$ [\d.]+,\d\d sem juros no cartão até 4x com juros$/.test(m.precoTexto), `${w}px: preco a vista e 2x, sem Pix ("${m.precoTexto}")`);

  /* Passar o mouse: a cor nao muda, o link cresce ~5% e a transicao e
     suave — medido contando quantas larguras DISTINTAS aparecem em 20
     quadros seguidos. Um salto seco da 2 valores; "2 FPS" daria 3 ou 4;
     uma transicao de 220 ms a 60 Hz da uma dezena. */
  const link = page.locator('[data-pp-wa]');
  // no celular a linha fica abaixo da dobra: sem rolar, o mouse "passa" fora da janela e nada acontece
  await link.scrollIntoViewIfNeeded();
  await page.waitForTimeout(100);
  const caixa = await link.boundingBox();
  await page.mouse.move(caixa.x + caixa.width / 2, caixa.y + caixa.height / 2);
  const quadros = await page.evaluate(() => new Promise((res) => {
    const a = document.querySelector('[data-pp-wa]'); const larguras = []; let n = 0;
    const tick = () => { larguras.push(+a.getBoundingClientRect().width.toFixed(1)); if (++n < 20) requestAnimationFrame(tick); else res(larguras); };
    requestAnimationFrame(tick);
  }));
  await page.waitForTimeout(300);
  const hover = await le();
  const distintas = new Set(quadros).size;
  const cresceu = (await link.boundingBox()).width / caixa.width;
  confere(hover.cor === m.cor, `${w}px hover: cor igual a de repouso (${hover.cor})`);
  confere(cresceu > 1.03 && cresceu < 1.07, `${w}px hover: cresce de leve (${(cresceu * 100 - 100).toFixed(1)}%)`);
  confere(distintas >= 5, `${w}px hover: transicao suave — ${distintas} larguras distintas em 20 quadros (${quadros[0]} → ${quadros[quadros.length - 1]})`);
  confere(!/steps|step-start|step-end/.test(m.transicao) && /transform/.test(m.transicao), `${w}px hover: transicao so no transform, sem steps (${m.transicao})`);

  // clicar (mouse pressionado) nao pode virar laranja; soltar fora para nao navegar
  await page.mouse.down();
  await page.waitForTimeout(80);
  const ativo = await le();
  await page.mouse.move(5, 5);
  await page.mouse.up();
  confere(ativo.cor === m.cor, `${w}px clique: cor igual a de repouso (${ativo.cor})`);
  await page.waitForTimeout(300);
  const solto = await link.boundingBox();
  confere(Math.abs(solto.width - caixa.width) < 0.6, `${w}px: volta ao tamanho de repouso (${solto.width.toFixed(1)} vs ${caixa.width.toFixed(1)})`);

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
  await page.screenshot({ path: `${saida}/estoque-${tema ? 'loja-' : ''}${w}.png`, clip: { x: 0, y: topo, width: w, height: Math.min(h - topo, r.height + 130) } });

  // sem numero: volta o status pela contagem, e nenhum link morto
  if (semNumero) {
    await page.goto(alvo(semNumero), { waitUntil: 'load' });
    const s = await page.evaluate(() => ({ link: !!document.querySelector('[data-pp-wa]'), texto: document.querySelector('[data-pp-stock]').innerText.trim() }));
    confere(!s.link && s.texto === 'Em estoque', `${w}px sem numero: volta "${s.texto}" sem link`);
    // e o estado sem estoque da bolinha: cinza, sem anel — a mesma regra de antes
    const cinza = await page.evaluate(() => { const d = document.querySelector('[data-pp-stock] .pp__dot'); d.classList.add('pp__dot--out'); const c = getComputedStyle(d); return c.backgroundColor + ' / ' + c.boxShadow + ' / ' + c.animationName; });
    confere(cinza === 'rgb(154, 154, 154) / none / none', `${w}px sem estoque: bolinha cinza estatica, sem pulso (${cinza})`);
  }
  await page.context().close();
}

await browser.close();
console.log(falhas.length ? `\n${falhas.length} falha(s)` : '\ntudo certo');
process.exit(falhas.length ? 1 : 0);
