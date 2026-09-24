/* Exporta os PNG e o .ico a partir dos SVG de marca/vetor/.
 *
 *   node marca/vetor/exportar.mjs
 *
 * Um tamanho novo e uma linha na tabela abaixo, e nunca um recorte a mao do
 * PNG original -- recorte a mao foi o que cortou a palavra «PhxSql» ao meio
 * na capa do dossie (marca/LEIA-ME.md). O Chromium do Playwright rasteriza;
 * o .ico e escrito aqui (cabecalho ICONDIR + PNG embutido, o formato que o
 * Windows aceita desde o Vista).
 */
import { chromium } from '/opt/node22/lib/node_modules/playwright/index.mjs';
import { readFileSync, writeFileSync, mkdirSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const AQUI = dirname(fileURLToPath(import.meta.url));
const SAIDA = join(AQUI, '..', 'derivados', 'vetor');

// [svg, arquivo, largura, fundo]
const TABELA = [
  ['phx-icone.svg', 'phx-icone-16.png', 16],
  ['phx-icone.svg', 'phx-icone-32.png', 32],
  ['phx-icone.svg', 'phx-icone-48.png', 48],
  ['phx-icone.svg', 'phx-icone-64.png', 64],
  ['phx-simbolo.svg', 'phx-simbolo-128.png', 128],
  ['phx-simbolo.svg', 'phx-simbolo-256.png', 256],
  ['phx-simbolo.svg', 'phx-simbolo-512.png', 512],
  // Android (launcher e loja) e iOS (apple-touch-icon): fundo da marca,
  // porque os dois sistemas recortam o icone e a transparencia vira preto.
  ['phx-icone.svg', 'android-192.png', 192, '#010418'],
  ['phx-icone.svg', 'android-512.png', 512, '#010418'],
  ['phx-icone.svg', 'ios-180.png', 180, '#010418'],
  ['phxsql-horizontal.svg', 'phxsql-horizontal-1200.png', 1200],
  ['phxzip-horizontal.svg', 'phxzip-horizontal-1200.png', 1200],
  ['phxmail-horizontal.svg', 'phxmail-horizontal-1200.png', 1200],
  ['phxblockchain-horizontal.svg', 'phxblockchain-horizontal-1200.png', 1200],
];
const ICO = [16, 32, 48, 256];

async function rasterizar(pagina, svgTexto, largura, fundo) {
  const m = svgTexto.match(/viewBox="[\d.-]+ [\d.-]+ ([\d.]+) ([\d.]+)"/);
  const altura = Math.round(largura * m[2] / m[1]);
  await pagina.setViewportSize({ width: largura, height: altura });
  await pagina.setContent(`<html><body style="margin:0;background:${fundo || 'transparent'}">`
    + svgTexto.replace('<svg ', `<svg width="${largura}" height="${altura}" `) + '</body></html>');
  return pagina.screenshot({ omitBackground: !fundo });
}

function ico(pngs) {
  // ICONDIR (6) + ICONDIRENTRY (16 cada) + os PNG, na ordem.
  const cab = Buffer.alloc(6 + 16 * pngs.length);
  cab.writeUInt16LE(0, 0); cab.writeUInt16LE(1, 2); cab.writeUInt16LE(pngs.length, 4);
  let desl = cab.length;
  pngs.forEach(([lado, png], i) => {
    const e = 6 + 16 * i;
    cab.writeUInt8(lado >= 256 ? 0 : lado, e);      // 0 quer dizer 256
    cab.writeUInt8(lado >= 256 ? 0 : lado, e + 1);
    cab.writeUInt16LE(1, e + 4);                     // planos
    cab.writeUInt16LE(32, e + 6);                    // bits por pixel
    cab.writeUInt32LE(png.length, e + 8);
    cab.writeUInt32LE(desl, e + 12);
    desl += png.length;
  });
  return Buffer.concat([cab, ...pngs.map(([, p]) => p)]);
}

mkdirSync(SAIDA, { recursive: true });
const nav = await chromium.launch();
const pagina = await nav.newPage();
for (const [svg, arq, larg, fundo] of TABELA) {
  const png = await rasterizar(pagina, readFileSync(join(AQUI, svg), 'utf8'), larg, fundo);
  writeFileSync(join(SAIDA, arq), png);
  console.log(arq, png.length, 'bytes');
}
const icone = readFileSync(join(AQUI, 'phx-icone.svg'), 'utf8');
const partes = [];
for (const lado of ICO) partes.push([lado, await rasterizar(pagina, icone, lado)]);
const bin = ico(partes);
writeFileSync(join(SAIDA, 'phx.ico'), bin);
console.log('phx.ico', bin.length, 'bytes,', ICO.join('/'), 'px');
await nav.close();
