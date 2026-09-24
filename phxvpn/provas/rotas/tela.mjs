// As redes alcancaveis exercitadas no navegador: o admin inclui (a recusa
// aparece, a rota aparece, a filial atras de um membro aparece), remove, e o
// membro comum ve sem poder mexer. Mede o que o CSS pode mentir: caixa alta
// no endereco e rolagem lateral no celular.
import { chromium } from '/opt/node22/lib/node_modules/playwright/index.mjs';

const URL = process.env.PAINEL, SAIDA = process.env.SAIDA;
const b = await chromium.launch();
const r = { data: new Date().toISOString().slice(0, 19) + 'Z' };
const erros = [];
async function pagina(largura) {
  const p = await b.newPage({ viewport: { width: largura, height: 900 } });
  p.on('pageerror', e => erros.push(e.message));
  p.on('console', m => { if (m.type() === 'error' && !/status of 4\d\d/.test(m.text())) erros.push(m.text()); });
  p.on('dialog', d => d.accept());
  return p;
}
const entrar = async (p, u, s) => {
  await p.goto(URL); await p.waitForSelector('#tela-login:not([hidden])');
  await p.fill('#f-login [name=usuario]', u); await p.fill('#f-login [name=senha]', s);
  await p.click('#f-login button[type=submit]');
  await p.waitForSelector('#tela-redes:not([hidden])', { timeout: 30000 });
  await p.waitForSelector('.rede', { timeout: 10000 });
};
const rotas = p => p.$$eval('.rota', ls => ls.map(l => l.querySelector('.cidr').textContent + ' | ' + l.querySelector('.onde').textContent));
const incluir = async (p, cidr, onde) => {
  await p.fill('.nova-rota input', cidr); await p.selectOption('.nova-rota select', onde);
  const antes = await p.$$eval('.rota', l => l.length);
  await p.click('.nova-rota button');
  await p.waitForFunction(n => document.querySelectorAll('.rota').length !== n || document.querySelector('[id^=m-rota-]')?.textContent, antes, { timeout: 15000 });
  await p.waitForTimeout(300);
};

const p = await pagina(900);
await entrar(p, 'admin', 'senha-admin-longa');
r.admin_ve_o_formulario = !!(await p.$('.nova-rota'));
r.opcoes = await p.$$eval('.nova-rota select option', o => o.map(x => x.textContent));
await incluir(p, '192.168.10.5/24', 'nat');
r.recusa_bit_de_host = await p.textContent('[id^=m-rota-]');
await incluir(p, '192.168.10.0/24', 'nat');
r.depois_de_incluir_nat = await rotas(p);
r.aviso_sem_openvpn = await p.textContent('#m-redes');
await incluir(p, '192.168.20.0/24', 'm:ana');
r.depois_de_incluir_filial = await rotas(p);
r.caixa_do_endereco = await p.$eval('.rota .cidr', e => getComputedStyle(e).textTransform);
r.fonte_do_endereco = await p.$eval('.rota .cidr', e => getComputedStyle(e).fontFamily);
r.cor_do_incluir = await p.$eval('.nova-rota button', e => getComputedStyle(e).borderColor);
r.cor_do_remover = await p.$eval('.rota button', e => getComputedStyle(e).borderColor);
await p.screenshot({ path: `${SAIDA}/tela-rotas-admin.png`, fullPage: true });
const cel = await pagina(390);
await entrar(cel, 'admin', 'senha-admin-longa');
r.rolagem_lateral_390 = await cel.evaluate(() => document.documentElement.scrollWidth - document.documentElement.clientWidth);
await cel.screenshot({ path: `${SAIDA}/tela-rotas-390.png`, fullPage: true });

const m = await pagina(900);
await entrar(m, 'ana', 'senha-da-ana-longa');
r.membro_ve = await rotas(m);
r.membro_ve_formulario = !!(await m.$('.nova-rota'));
r.membro_ve_remover = (await m.$$('.rota button')).length;

const antes = (await rotas(p)).length;
await p.click('.rota button');
// A confirmacao e dialogo DENTRO da pagina (nunca o confirm() do navegador).
await p.waitForSelector('#d-confirmar[open]');
r.dialogo_de_remover = await p.textContent('#dc-texto');
await p.click('#dc-ok');
await p.waitForFunction(n => document.querySelectorAll('.rota').length === n - 1, antes, { timeout: 15000 });
r.depois_de_remover = await rotas(p);
r.erros_de_pagina = erros;
r.passou = r.admin_ve_o_formulario && /192\.168\.10\.0\/24/.test(r.recusa_bit_de_host)
  && r.depois_de_incluir_filial.length === 2 && r.depois_de_incluir_filial.some(x => x.includes('atrás de ana'))
  && r.caixa_do_endereco === 'none' && r.rolagem_lateral_390 <= 0
  && r.membro_ve.length === 2 && !r.membro_ve_formulario && r.membro_ve_remover === 0
  && r.depois_de_remover.length === 1 && erros.length === 0;
console.log(JSON.stringify(r, null, 1));
await b.close();
