/* Mede a vitrine da coleção: o selo passa da borda da imagem? o banner
   corta quanto da foto no celular?
 *
 * Nasceu de um defeito real (17/09/2026): o selo "5% no Pix" ficava 6px
 * para fora da borda direita da imagem e 4px acima do topo, meio em cima
 * da foto e meio no branco do cartão — e antes disso um remendo foi ao ar
 * mirando a View Transition, que nesta loja NUNCA roda. Ler o código não
 * pegou nem um nem outro; medir pegou os dois.
 *
 * Uso:  node mock/mede-vitrine.mjs <pagina.html> [--com-remendo <arquivo.css>]
 * Sai com código 1 se algo passar da borda ou se o corte do banner
 * ficar acima do teto.
 */
import { chromium } from '/opt/node22/lib/node_modules/playwright/index.mjs';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

/* O defeito do banner era BINARIO: a foto ocupava 356x200 no topo de uma
   moldura de 356x480 e os 280px de baixo ficavam brancos, com o titulo
   metade fora da foto (<picture> sem altura mata o `height:100%` do <img>).
   Por isso a reprovacao e "nao preenche", nao "corta demais".
   O corte e informacao: foto deitada em tela em pe sempre perde largura, e
   o teto abaixo so pega uma volta ao desenho antigo de moldura fixa alta. */
const TETO_CORTE = 45;
const LARGURAS = [[390, 844], [1280, 900]];

const pagina = process.argv[2];
if (!pagina) { console.error('falta a pagina'); process.exit(2); }
const iRem = process.argv.indexOf('--com-remendo');
const remendo = iRem > 0 ? readFileSync(process.argv[iRem + 1], 'utf8') : null;

const nav = await chromium.launch();
let reprovou = 0;

for (const [larg, alt] of LARGURAS) {
  const pag = await nav.newPage({ viewport: { width: larg, height: alt },
    deviceScaleFactor: 2, isMobile: larg < 500, hasTouch: larg < 500 });
  await pag.goto('file://' + resolve(pagina));
  if (remendo) await pag.addStyleTag({ content: remendo });
  await pag.waitForTimeout(800);

  const r = await pag.evaluate((teto) => {
    const fora = [];
    for (const c of document.querySelectorAll('product-card')) {
      c.scrollIntoView({ block: 'center' });
      const gal = c.querySelector('.card-gallery')?.getBoundingClientRect();
      const selo = c.querySelector('.product-badges')?.getBoundingClientRect();
      if (!gal || !selo) continue;
      const vaza = { dir: Math.round(selo.right - gal.right), esq: Math.round(gal.left - selo.left),
                     topo: Math.round(gal.top - selo.top), base: Math.round(selo.bottom - gal.bottom) };
      const pior = Math.max(vaza.dir, vaza.esq, vaza.topo, vaza.base);
      if (pior > 0) fora.push({ titulo: c.querySelector('h2,h3')?.textContent.trim().slice(0, 20), vaza, pior });
    }
    const banners = [];
    for (const img of document.querySelectorAll('.wx-hero__media img')) {
      const q = img.getBoundingClientRect(), nW = img.naturalWidth, nH = img.naturalHeight;
      if (!nW || !q.width) continue;
      const esc = Math.max(q.width / nW, q.height / nH);
      const vis = Math.min(1, q.width / (nW * esc)) * Math.min(1, q.height / (nH * esc));
      const cai = img.closest('.wx-hero__slide') || img.closest('.wx-hero');
      const cb = cai?.getBoundingClientRect();
      banners.push({ natural: `${nW}x${nH}`, moldura: `${Math.round(q.width)}x${Math.round(q.height)}`,
                     caixa: cb && `${Math.round(cb.width)}x${Math.round(cb.height)}`,
                     preenche: !cb || Math.round(q.height) >= Math.round(cb.height) - 4,
                     corte: Math.round((1 - vis) * 100) });
    }
    return { fora, banners, teto };
  }, TETO_CORTE);

  console.log(`\n### ${larg}x${alt}`);
  if (r.fora.length) {
    reprovou++;
    console.log(`  SELO FORA DA IMAGEM em ${r.fora.length} cartao(oes):`);
    for (const f of r.fora) console.log(`    ${(f.titulo || '?').padEnd(22)} passa ${f.pior}px  (${JSON.stringify(f.vaza)})`);
  } else {
    console.log('  selo: dentro da imagem em todos os cartoes  ok');
  }
  for (const b of r.banners) {
    const ruim = !b.preenche || b.corte > TETO_CORTE;
    if (ruim) reprovou++;
    const faixa = b.preenche ? '' : `  NAO PREENCHE a caixa ${b.caixa} — sobra faixa vazia`;
    const demais = b.corte > TETO_CORTE ? `  CORTE ACIMA DO TETO (${TETO_CORTE}%)` : '';
    console.log(`  banner ${b.natural} em ${b.moldura}: corta ${b.corte}%${faixa}${demais}${ruim ? '' : '  ok'}`);
  }
  if (!r.banners.length) console.log('  banner: nenhuma imagem carregada (nao medido)');
  await pag.close();
}
await nav.close();
console.log(reprovou ? `\n${reprovou} reprovacao(oes)` : '\ntudo dentro');
process.exit(reprovou ? 1 : 0);
