// O force-cookie na tela (uso: TELA=cookie.mjs ./tela.sh): o admin ve o
// bloco desligado com o aviso, liga e ve «ligado»; a ana nao ve o bloco.
import { chromium } from '/opt/node22/lib/node_modules/playwright/index.mjs';

const URL = process.env.PAINEL, SAIDA = process.env.SAIDA;
const b = await chromium.launch();
const r = { erros: [] };
const p = await b.newPage({ viewport: { width: 1280, height: 900 } });
p.on('pageerror', e => r.erros.push(e.message));
p.on('console', m => { if (m.type() === 'error' && !/status of 40[01]/.test(m.text())) r.erros.push(m.text()); });
p.on('dialog', d => { r.erros.push('dialogo nativo: ' + d.type()); d.dismiss(); });
const entrar = async (u, s) => {
  await p.goto(URL); await p.evaluate(() => sessionStorage.clear()); await p.goto(URL);
  await p.waitForSelector('#tela-login:not([hidden])');
  await p.fill('#f-login [name=usuario]', u); await p.fill('#f-login [name=senha]', s);
  await p.click('#f-login button[type=submit]');
  await p.waitForSelector('#tela-redes:not([hidden])');
  await p.waitForTimeout(1500);
};
const bloco = () => p.$$eval('.saida .titulo', l => l.filter(x => /force-cookie/.test(x.textContent)).map(x => x.parentElement.textContent));
await entrar('admin', 'senha-admin-longa');
r.admin_antes = await bloco();
await p.screenshot({ path: `${SAIDA}/32-force-cookie-desligado.png`, fullPage: true });
await p.click('text=Ligar force-cookie');
await p.waitForFunction(() => /force-cookie ligado/.test(document.querySelector('#m-redes').textContent));
await p.waitForTimeout(800);
r.admin_depois = await bloco();
await p.screenshot({ path: `${SAIDA}/32-force-cookie-ligado.png`, fullPage: true });
await entrar('ana', 'senha-da-ana-longa');
r.ana = await bloco();
await b.close();
console.log(JSON.stringify(r, null, 1));
const ok = r.admin_antes.length === 1 && /desligado \(padrão\)/.test(r.admin_antes[0]) && /anteriores à 2\.6/.test(r.admin_antes[0])
  && r.admin_depois.length === 1 && /ligado/.test(r.admin_depois[0]) && /Desligar force-cookie/.test(r.admin_depois[0])
  && r.ana.length === 0 && r.erros.length === 0;
console.log(ok ? 'TELA: confere' : 'TELA: NAO confere');
process.exit(ok ? 0 : 1);
