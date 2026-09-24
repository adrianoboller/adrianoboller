/* A folha de marca CORRIGIDA: troca a promessa falsa «ACID compliant».
 *
 *   node marca/vetor/corrigir-folha.mjs
 *
 * O original (`marca/phxsql-manual-de-marca.png`) fica como veio -- ele e o
 * registro do que o dono recebeu. O derivado corrigido sai em
 * `marca/derivados/phxsql-manual-de-marca-corrigido.png`, e e ele que vai
 * para cliente.
 *
 * Por que a frase nova e a que e (docs/ACID.md): o I entregue por padrao e
 * READ COMMITTED, entao «ACID compliant» nao se escreve (CONTRATO-1.0 §2.1).
 * E «durable» sem ressalva tambem nao: no regime padrao uma escrita comum
 * responde OK sem `fsync` (§5.1). O que e verdade medida e a transacao:
 * COMMIT, ROLLBACK e SAVEPOINT, desde o pedido 162.
 *
 * O remendo pinta por cima com a cor de fundo AMOSTRADA ao lado do bloco e
 * escreve com a Exo 2 embutida (marca/fontes) -- a mesma voz da folha.
 */
import { chromium } from '/opt/node22/lib/node_modules/playwright/index.mjs';
import { readFileSync, writeFileSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const AQUI = dirname(fileURLToPath(import.meta.url));
const MARCA = join(AQUI, '..');
const original = readFileSync(join(MARCA, 'phxsql-manual-de-marca.png')).toString('base64');
const exo2 = readFileSync(join(MARCA, 'fontes', 'exo2-latin-var.woff2')).toString('base64');

const nav = await chromium.launch();
const p = await nav.newPage({ viewport: { width: 1448, height: 1086 } });
await p.setContent(`<html><head><style>
@font-face{font-family:'Exo 2';font-weight:400 700;src:url(data:font/woff2;base64,${exo2}) format('woff2')}
body{margin:0}</style></head><body><canvas id="c" width="1448" height="1086"></canvas></body></html>`);
const png = await p.evaluate(async ([b64]) => {
  await document.fonts.load("600 15px 'Exo 2'");
  await document.fonts.load("400 15px 'Exo 2'");
  const img = new Image();
  img.src = 'data:image/png;base64,' + b64;
  await img.decode();
  const c = document.getElementById('c');
  const g = c.getContext('2d');
  g.drawImage(img, 0, 0);
  // Fundo amostrado a direita do bloco, na mesma faixa (antes do divisor).
  const [r, gg, b] = g.getImageData(330, 1020, 1, 1).data;
  g.fillStyle = `rgb(${r},${gg},${b})`;
  g.fillRect(140, 986, 205, 70);
  g.fillStyle = '#DDE2EB';
  // 14 px e 0,3 px de espaco: com 15 px a palavra encostava no divisor.
  g.font = "600 14px 'Exo 2'";
  g.letterSpacing = '0.3px';
  g.fillText('TRANSACTIONAL STORAGE', 148, 1004);
  g.letterSpacing = '0px';
  g.fillStyle = '#a8b0c0';
  g.font = "400 15px 'Exo 2'";
  g.fillText('Commit, rollback', 148, 1026);
  g.fillText('and savepoints.', 148, 1046);
  return c.toDataURL('image/png').split(',')[1];
}, [original]);
const destino = join(MARCA, 'derivados', 'phxsql-manual-de-marca-corrigido.png');
writeFileSync(destino, Buffer.from(png, 'base64'));
console.log('gravado', destino);
await nav.close();
