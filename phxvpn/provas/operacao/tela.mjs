// A tela do historico de conexoes exercitada no navegador: o admin ve as tres
// conexoes (as de todos) e muda a retencao; a ana ve so a dela. Monitor e
// celular, zero erro de console, nenhum dialogo nativo.
import { chromium } from '/opt/node22/lib/node_modules/playwright/index.mjs';

const URL = process.env.PAINEL, SAIDA = process.env.SAIDA;
const b = await chromium.launch();
const r = { erros: [] };
for (const [nome, largura, altura] of [['monitor', 1280, 800], ['celular', 390, 844]]) {
  const p = await b.newPage({ viewport: { width: largura, height: altura } });
  p.on('pageerror', e => r.erros.push(e.message));
  p.on('console', m => { if (m.type() === 'error' && !/status of 40[01]/.test(m.text())) r.erros.push(m.text()); });
  p.on('dialog', d => { r.erros.push('dialogo nativo: ' + d.type()); d.dismiss(); });
  const entrar = async (u, s) => {
    await p.goto(URL);
    await p.evaluate(() => sessionStorage.clear());
    await p.goto(URL);
    await p.waitForSelector('#tela-login:not([hidden])');
    await p.fill('#f-login [name=usuario]', u);
    await p.fill('#f-login [name=senha]', s);
    await p.click('#f-login button[type=submit]');
    await p.waitForSelector('#tela-redes:not([hidden])');
    await p.click('#b-historico');
    await p.waitForSelector('#tela-historico:not([hidden])');
    await p.waitForFunction(() => document.querySelectorAll('#t-historico tr').length > 0);
  };
  await entrar('admin', 'senha-admin-longa');
  r[nome + '_admin_linhas'] = await p.$$eval('#t-historico tr', l => l.length);
  r[nome + '_admin_escopo'] = await p.textContent('#h-escopo');
  r[nome + '_admin_ve_retencao'] = await p.isVisible('#f-retencao');
  r[nome + '_primeira_linha'] = await p.$$eval('#t-historico tr:first-child td', l => l.map(x => x.textContent));
  await p.screenshot({ path: `${SAIDA}/31-historico-${nome}-admin.png`, fullPage: true });
  if (nome === 'monitor') {
    await p.fill('#f-retencao [name=dias]', '30');
    await p.click('#f-retencao button[type=submit]');
    await p.waitForFunction(() => /retenção de 30 dias/.test(document.querySelector('#m-retencao').textContent));
    r.retencao_msg = await p.textContent('#m-retencao');
    r.linhas_depois_de_30_dias = await p.$$eval('#t-historico tr', l => l.length);
    await p.screenshot({ path: `${SAIDA}/31-historico-monitor-retencao.png`, fullPage: true });
  }
  await entrar('ana', 'senha-da-ana-longa');
  r[nome + '_ana_linhas'] = await p.$$eval('#t-historico tr', l => l.length);
  r[nome + '_ana_logins'] = await p.$$eval('#t-historico tr td:first-child', l => [...new Set(l.map(x => x.textContent))]);
  r[nome + '_ana_ve_retencao'] = await p.isVisible('#f-retencao');
  // Celular: a tabela rola DENTRO do cartao, nunca a pagina.
  r[nome + '_pagina_rola_de_lado'] = await p.evaluate(() => document.documentElement.scrollWidth > window.innerWidth);
  await p.screenshot({ path: `${SAIDA}/31-historico-${nome}-ana.png`, fullPage: true });
  await p.close();
}
await b.close();
console.log(JSON.stringify(r, null, 1));
const ok = r.monitor_admin_linhas === 3 && r.monitor_ana_linhas === 1 && r.celular_ana_linhas === 1
  && r.monitor_admin_ve_retencao && !r.monitor_ana_ve_retencao
  && JSON.stringify(r.monitor_ana_logins) === '["ana"]'
  && r.linhas_depois_de_30_dias === 2 && !r.celular_pagina_rola_de_lado && r.erros.length === 0;
console.log(ok ? 'TELA: confere' : 'TELA: NAO confere');
process.exit(ok ? 0 : 1);
