/* Exercita o placar de sections/wx-avaliacoes.liquid com avaliacao — o ramo
 * que a loja ainda nao consegue mostrar (sem avaliacao publicada ate
 * 28/09/2026) e que nao se testa gravando avaliacao falsa.
 *
 * Nasceu de um defeito achado aqui: nota 5,00 saia "5" (round: 1 de numero
 * inteiro escreve "5" num motor e "5.0" no outro). Hoje a nota sai por
 * aritmetica inteira de decimos.
 *
 * Uso (de engineprint/shopify, com `npm i liquidjs@10`):
 *   node mock/placar-avaliacoes.mjs
 * Sai com codigo 1 se algum placar divergir do esperado.
 */
import { Liquid } from 'liquidjs';
import { readFileSync } from 'node:fs';
import { resolve, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const aqui = dirname(fileURLToPath(import.meta.url));
let src = readFileSync(resolve(aqui, '../sections/wx-avaliacoes.liquid'), 'utf8');
src = src.replace(/{%\s*stylesheet\s*%}[\s\S]*?{%\s*endstylesheet\s*%}/, '').replace(/{%\s*schema\s*%}[\s\S]*?{%\s*endschema\s*%}/, '');

// [nota gravada pelo app, total gravado, texto esperado, largura das estrelas cheias]
const CASOS = [
  ['4.80', '2', '4,8 de 5, com base em 2 avaliações', '96'],
  ['5.00', '1', '5,0 de 5, com base em 1 avaliação', '100'],
  ['4.25', '12', '4,3 de 5, com base em 12 avaliações', '85'],
  ['3.96', '7', '4,0 de 5, com base em 7 avaliações', '79.2'],
  ['1.00', '3', '1,0 de 5, com base em 3 avaliações', '20'],
  ['0.00', '0', 'Ainda não há avaliações publicadas.', null],
];

const motor = new Liquid();
let falhas = 0;
for (const [nota, total, esperado, largura] of CASOS) {
  const html = await motor.parseAndRender(src, {
    shop: { metafields: { judgeme: { all_reviews_rating: nota, all_reviews_count: total, all_reviews_header: '' } } },
    page: { title: 'Avaliações', content: '' },
    section: { settings: { olho: '', texto_vazio: 'Ainda não há avaliações publicadas.' } },
  });
  const bloco = html.match(/<div class="wxav__placar">([\s\S]*?)<\/div>\s*<div class="wxav__lista">/)[1];
  const texto = bloco.replace(/<span class="wxav__estrelas[\s\S]*?<\/span>\s*<\/span>/, '').replace(/<[^>]+>/g, ' ').replace(/\s+/g, ' ').trim();
  const larg = (bloco.match(/width: ([\d.]+)%/) || [])[1] || null;
  const ok = texto === esperado && (largura === null ? larg === null : Number(larg) === Number(largura));
  if (!ok) falhas++;
  console.log(`${ok ? 'ok    ' : 'FALHA '}nota ${nota}, ${total}: "${texto}" estrelas ${larg ?? '-'}%${ok ? '' : ` (esperado "${esperado}" ${largura ?? '-'}%)`}`);
}
process.exit(falhas ? 1 : 0);
