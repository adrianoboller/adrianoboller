// Ajudantes do video: legenda, cursor visivel, marcas de cena e espera por
// arquivo entre as janelas (cada uma roda no seu `ip netns`).
import { chromium } from '/opt/node22/lib/node_modules/playwright/index.mjs';
import fs from 'fs';
export const D = process.env.DEMO || '/var/tmp/phx-demo';
export const LARG = 1280, ALT = 720;
export const pausa = (ms) => new Promise((r) => setTimeout(r, ms));
export const espera = async (f, ms = 180000) => {
  const t = Date.now();
  while (!fs.existsSync(`${D}/${f}`)) { if (Date.now() - t > ms) throw new Error('esperei ' + f); await pausa(150); }
};
export const marca = (f, t = '') => fs.writeFileSync(`${D}/${f}`, t);

export async function abrir(quem) {
  const b = await chromium.launch();
  const ctx = await b.newContext({ viewport: { width: LARG, height: ALT }, recordVideo: { dir: `${D}/video-${quem}`, size: { width: LARG, height: ALT } } });
  const t0 = Date.now();
  const p = await ctx.newPage();
  const erros = []; p.on('pageerror', (e) => erros.push(e.message));
  await p.goto(fs.readFileSync(`${D}/url-${quem}`, 'utf8').trim());
  const cenas = [];
  let atual = null;
  return {
    b, ctx, p, erros,
    // Cena: o trecho do video desta janela que entra no filme, com a legenda.
    async cena(nome, texto) {
      if (atual) this.fim();
      await legenda(p, texto);
      atual = { nome, ini: (Date.now() - t0) / 1000 };
    },
    fim() { if (atual) { atual.fim = (Date.now() - t0) / 1000; cenas.push(atual); atual = null; } },
    async fechar() {
      this.fim(); await legenda(p, '');
      fs.writeFileSync(`${D}/cenas-${quem}.json`, JSON.stringify(cenas, null, 1));
      const v = await p.video().path(); await ctx.close(); await b.close();
      fs.renameSync(v, `${D}/bruto-${quem}.webm`);
      console.log(quem, 'erros', JSON.stringify(erros));
    },
  };
}

// Legenda na parte de baixo, sobre a janela (so para o video).
export async function legenda(p, texto) {
  await p.evaluate((t) => {
    let l = document.getElementById('__legenda');
    if (!l) {
      l = document.createElement('div'); l.id = '__legenda';
      Object.assign(l.style, { position: 'fixed', left: '50%', bottom: '44px', transform: 'translateX(-50%)', zIndex: 99999,
        background: 'rgba(1,4,24,.92)', color: '#fff', border: '1px solid #FF4D10', borderRadius: '10px', padding: '10px 18px',
        font: '600 20px/1.35 "Exo 2",system-ui,sans-serif', maxWidth: '86%', textAlign: 'center', pointerEvents: 'none',
        boxShadow: '0 6px 24px rgba(0,0,0,.5)' });
      document.body.appendChild(l);
    }
    l.textContent = t; l.style.display = t ? 'block' : 'none';
  }, texto);
}

// Cursor visivel: o headless nao desenha o do sistema.
async function cursor(p, x, y) {
  await p.evaluate(([x, y]) => {
    let c = document.getElementById('__cursor');
    if (!c) {
      c = document.createElement('div'); c.id = '__cursor';
      Object.assign(c.style, { position: 'fixed', width: '22px', height: '22px', borderRadius: '50%', zIndex: 100000,
        border: '3px solid #FF4D10', background: 'rgba(255,77,16,.25)', pointerEvents: 'none',
        transition: 'left .45s ease, top .45s ease', left: '640px', top: '360px' });
      document.body.appendChild(c);
    }
    c.style.left = (x - 11) + 'px'; c.style.top = (y - 11) + 'px';
  }, [x, y]);
  await pausa(550);
}

export async function clicar(p, seletor) {
  const e = p.locator(seletor).first();
  await e.waitFor({ state: 'visible', timeout: 60000 });
  const r = await e.boundingBox();
  await cursor(p, r.x + r.width / 2, r.y + r.height / 2);
  await e.click(); await pausa(350);
}

export async function digitar(p, seletor, texto, atraso = 45) {
  await clicar(p, seletor);
  await p.locator(seletor).first().pressSequentially(texto, { delay: atraso });
}
