/* Troca as imagens da marca embutidas no Centro de Controle pelas do vetor.
 *
 *   node marca/vetor/embutir-na-tela.mjs
 *
 * Decisao do dono, 24/09/2026: o vetor e a marca oficial. A pagina e um
 * arquivo so (data URI, sem de onde buscar imagem), entao cada PNG embutido
 * no `crates/phxsql-server/ui/index.html` se refaz aqui, do SVG, NO MESMO
 * TAMANHO de caixa que ja ocupava -- o desenho entra centrado ("contain"), e
 * nenhum layout da tela muda. Os quatro lugares sao achados pelo texto que
 * vem antes do data URI, e o script recusa se um deles nao aparecer uma vez
 * so: trocar a imagem errada seria pior que nao trocar.
 */
import { chromium } from '/opt/node22/lib/node_modules/playwright/index.mjs';
import { readFileSync, writeFileSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const AQUI = dirname(fileURLToPath(import.meta.url));
const PAGINA = join(AQUI, '..', '..', 'crates', 'phxsql-server', 'ui', 'index.html');

// [texto antes do data URI, svg, largura, altura]
const LUGARES = [
  ['<link rel="icon" type="image/png" href="', 'phx-icone.svg', 32, 32],
  ['<img class="simbolo" src="', 'phx-simbolo.svg', 224, 133],
  ['<span class="marca"><img src="', 'phx-icone.svg', 72, 62],
  ['const SIMBOLO_PHOENIX = "', 'phx-simbolo.svg', 224, 133],
];

const nav = await chromium.launch();
const p = await nav.newPage();
let html = readFileSync(PAGINA, 'utf8');
for (const [antes, svg, w, h] of LUGARES) {
  const re = new RegExp(antes.replace(/[.*+?^${}()|[\]\\]/g, '\\$&') + 'data:image/png;base64,[A-Za-z0-9+/=]+', 'g');
  const achados = html.match(re) || [];
  if (achados.length !== 1) throw new Error(`«${antes}» aparece ${achados.length} vezes: recusado`);
  const texto = readFileSync(join(AQUI, svg), 'utf8');
  await p.setViewportSize({ width: w, height: h });
  await p.setContent(`<html><body style="margin:0;width:${w}px;height:${h}px;display:flex;align-items:center;justify-content:center">`
    + texto.replace('<svg ', `<svg style="max-width:${w}px;max-height:${h}px;width:100%;height:100%" preserveAspectRatio="xMidYMid meet" `) + '</body></html>');
  const png = await p.screenshot({ omitBackground: true });
  html = html.replace(re, antes + 'data:image/png;base64,' + png.toString('base64'));
  console.log(`${svg} -> ${w}x${h}, ${png.length} bytes, em «${antes.slice(0, 40)}»`);
}
writeFileSync(PAGINA, html);
await nav.close();
