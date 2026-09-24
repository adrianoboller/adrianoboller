// A tela do autenticador exercitada no navegador: cadastrar pelo QR/chave,
// entrar com e sem codigo, o dono exigindo na rede, o admin vendo e zerando.
// O codigo sai do `crypto` do Node -- outra implementacao, independente.
import { chromium } from '/opt/node22/lib/node_modules/playwright/index.mjs';
import crypto from 'crypto';

const URL = process.env.PAINEL, SAIDA = process.env.SAIDA;
function totp(b32, desloc = 0) {
  const A = 'ABCDEFGHIJKLMNOPQRSTUVWXYZ234567'; let bits = '';
  for (const c of b32.replace(/[\s=]/g, '').toUpperCase()) bits += A.indexOf(c).toString(2).padStart(5, '0');
  const chave = Buffer.from(bits.match(/.{8}/g).map(b => parseInt(b, 2)));
  const t = Buffer.alloc(8); t.writeBigUInt64BE(BigInt(Math.floor((Date.now() / 1000 + desloc) / 30)));
  const h = crypto.createHmac('sha1', chave).update(t).digest(), o = h[19] & 15;
  return String((h.readUInt32BE(o) & 0x7fffffff) % 1e6).padStart(6, '0');
}
const b = await chromium.launch();
const p = await b.newPage({ viewport: { width: 900, height: 900 } });
const erros = []; p.on('pageerror', e => erros.push(e.message));
p.on('console', m => { if (m.type() === 'error' && !/status of 40[01]/.test(m.text())) erros.push(m.text()); });
p.on('dialog', d => d.accept());
const r = {};
const entrar = async (u, s, c = '') => {
  await p.fill('#f-login [name=usuario]', u); await p.fill('#f-login [name=senha]', s);
  await p.fill('#f-login [name=codigo]', c); await p.$eval('#m-login', m => { m.textContent = ''; });
  await p.click('#f-login button[type=submit]');
  // O PBKDF2 leva segundos no binario de depuracao: espera a RESPOSTA.
  await p.waitForFunction(() => !document.querySelector('#tela-redes').hidden || document.querySelector('#m-login').textContent, null, { timeout: 30000 });
};
await p.goto(URL); await p.waitForSelector('#tela-login:not([hidden])');
await entrar('ana', 'senha-da-ana-longa');
await p.click('#b-mfa'); await p.waitForSelector('#tela-mfa:not([hidden])');
r.estado_antes = await p.textContent('#mfa-estado');
await p.click('#b-mfa-iniciar'); await p.waitForSelector('#mfa-novo:not([hidden])');
const segredo = (await p.textContent('#mfa-segredo')).trim();
r.segredo_caracteres = segredo.replace(/\s/g, '').length;
r.qr_desenhado = await p.$eval('#mfa-qr', d => { const s = d.querySelector('svg'); return s ? s.getBoundingClientRect().width : 0; });
await p.screenshot({ path: `${SAIDA}/tela-mfa-cadastro.png`, fullPage: true });
await p.fill('#f-mfa [name=codigo]', '000000'); await p.click('#b-mfa-ok'); await p.waitForTimeout(700);
r.codigo_errado = await p.textContent('#m-mfa');
await p.fill('#f-mfa [name=codigo]', totp(segredo)); await p.click('#b-mfa-ok'); await p.waitForTimeout(700);
r.confirmado = await p.textContent('#m-mfa');
r.estado_depois = await p.textContent('#mfa-estado');
await p.click('#b-logout'); await p.waitForSelector('#tela-login:not([hidden])');
await entrar('ana', 'senha-da-ana-longa');
r.login_sem_codigo = await p.textContent('#m-login');
await entrar('ana', 'senha-da-ana-longa', totp(segredo, 30));
r.login_com_codigo = await p.isVisible('#tela-redes');
await p.click('#b-logout'); await p.waitForSelector('#tela-login:not([hidden])');
await entrar('admin', 'senha-admin-longa');
await p.click('text=Exigir autenticador'); await p.waitForTimeout(1500);
r.rede_info = await p.textContent('#lista-redes .info');
r.aviso = await p.textContent('#m-redes');
await p.screenshot({ path: `${SAIDA}/tela-mfa-rede.png`, fullPage: true });
await p.click('#b-admin'); await p.waitForTimeout(800);
r.admin_linha_ana = await p.$$eval('#t-usuarios tr', l => l.map(x => x.textContent).find(t => t.startsWith('ana')));
await p.screenshot({ path: `${SAIDA}/tela-mfa-admin.png`, fullPage: true });
await p.click('#t-usuarios button.exclui'); await p.waitForTimeout(800);
r.admin_depois_de_zerar = await p.$$eval('#t-usuarios tr', l => l.map(x => x.textContent).find(t => t.startsWith('ana')));
r.erros_de_console = erros;
console.log(JSON.stringify(r, null, 1));
await b.close();
