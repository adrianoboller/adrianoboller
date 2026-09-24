// PNGs da marca a partir dos SVG (Chromium do Playwright), fundo transparente.
import { chromium } from '/opt/node22/lib/node_modules/playwright/index.mjs';
import fs from 'fs'; import path from 'path';
const AQUI = path.dirname(new URL(import.meta.url).pathname);
const pecas = [['simbolo', 1024, 1024], ['simbolo', 256, 256], ['icone-app', 512, 512], ['icone-app', 256, 256],
  ['icone-app', 64, 64], ['icone-app', 32, 32], ['logo-horizontal', 2400, 720], ['logo-vertical', 1600, 1800]];
fs.mkdirSync(`${AQUI}/png`, { recursive: true });
const b = await chromium.launch();
for (const [nome, w, h] of pecas) {
  const p = await b.newPage({ viewport: { width: w, height: h } });
  const html = `${AQUI}/png/.tmp.html`;
  fs.writeFileSync(html, `<body style="margin:0;background:transparent"><img src="../${nome}.svg" style="width:${w}px;height:${h}px;display:block"></body>`);
  await p.goto('file://' + html); await p.waitForTimeout(400);
  await p.screenshot({ path: `${AQUI}/png/${nome}-${w}.png`, omitBackground: true }); await p.close();
  fs.unlinkSync(html);
}
await b.close(); console.log(fs.readdirSync(`${AQUI}/png`).join(' '));
