import { chromium } from '/opt/node22/lib/node_modules/playwright/index.mjs';
import fs from 'fs';
export const D = '/var/tmp/mesa-usb';
export const S = process.env.SAIDA;
export const espera = async (f, ms = 60000) => { const t = Date.now(); while (!fs.existsSync(`${D}/${f}`)) { if (Date.now() - t > ms) throw new Error('esperei ' + f); await new Promise(r => setTimeout(r, 200)); } };
export const marca = (f, t = '') => fs.writeFileSync(`${D}/${f}`, t);
export async function abrir(url) {
  const b = await chromium.launch(); const p = await b.newPage({ viewport: { width: 620, height: 820 } });
  const erros = []; p.on('pageerror', e => erros.push(e.message)); p.on('console', m => { if (m.type() === 'error') erros.push(m.text()); });
  await p.goto(url); return { b, p, erros };
}
