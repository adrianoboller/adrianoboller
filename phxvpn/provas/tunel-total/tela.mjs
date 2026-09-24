// A saida da rede exercitada no navegador: o admin liga o tunel total (a
// recusa sem DNS aparece; com os nomes, entra), o membro comum ve sem poder
// mexer, e o dialogo de entrar manda o que a maquina pediu (DNS no Linux, sem
// IPv6) -- conferido no perfil que volta. Mede o que o CSS pode mentir: a
// caixa de marcar esticada pelo `.campo input{width:100%}`, caixa alta na
// zona e rolagem lateral no celular.
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
const estado = p => p.textContent('.saida .estado');
const salvar = async (p, marcar) => {
  for (const [nome, v] of Object.entries(marcar)) {
    const c = await p.$(`.saida [name=${nome}]`);
    if ((await c.isChecked()) !== v && !(await c.isDisabled())) await c.click();
  }
  const resp = p.waitForResponse(x => x.url().endsWith('/api/redes/saida/definir'));
  await p.click('.saida button[type=submit]');
  const x = await resp;
  await p.waitForTimeout(600);
  return x.status();
};

const p = await pagina(900);
await entrar(p, 'admin', 'senha-admin-longa');
r.admin_ve_o_bloco = !!(await p.$('.saida form'));
r.estado_inicial = await estado(p);
r.largura_da_caixa_de_marcar = await p.$eval('.saida [name=tunel_total]', e => e.getBoundingClientRect().width);
r.block_local_travado_sem_tunel = await p.$eval('.saida [name=bloquear_local]', e => e.disabled);
r.status_tunel_sem_dns = await salvar(p, { tunel_total: true, dns_nomes: false });
r.recusa_sem_dns = await p.textContent('[id^=m-saida-]');
r.status_liga = await salvar(p, { tunel_total: true, dns_nomes: true, bloquear_local: true });
r.estado_ligado = await estado(p);
r.aviso = await p.textContent('#m-redes');
r.caixa_da_zona = await p.$eval('.saida .zona', e => getComputedStyle(e).textTransform);
r.fonte_da_zona = await p.$eval('.saida .zona', e => getComputedStyle(e).fontFamily);
r.cor_do_salvar = await p.$eval('.saida button[type=submit]', e => getComputedStyle(e).borderColor);
await p.screenshot({ path: `${SAIDA}/tela-saida-admin.png`, fullPage: true });
const cel = await pagina(390);
await entrar(cel, 'admin', 'senha-admin-longa');
r.rolagem_lateral_390 = await cel.evaluate(() => document.documentElement.scrollWidth - document.documentElement.clientWidth);
await cel.screenshot({ path: `${SAIDA}/tela-saida-390.png`, fullPage: true });

const m = await pagina(900);
await entrar(m, 'ana', 'senha-da-ana-longa');
r.membro_ve_estado = await estado(m);
r.membro_ve_formulario = !!(await m.$('.saida form'));
// O dialogo de entrar: o que a maquina pede vai no pedido, e volta no perfil.
await m.click('#b-entrar');
await m.fill('#f-rede [name=nome]', 'Matriz'); await m.fill('#f-rede [name=senha]', 'senha-da-rede');
await m.selectOption('#f-rede [name=dns_linux]', 'systemd-resolved');
await m.check('#f-rede [name=sem_ipv6]');
r.largura_sem_ipv6 = await m.$eval('#f-rede [name=sem_ipv6]', e => e.getBoundingClientRect().width);
await m.screenshot({ path: `${SAIDA}/tela-entrar-maquina.png` });
const pedido = m.waitForRequest(x => x.url().endsWith('/api/redes/entrar'));
const resposta = m.waitForResponse(x => x.url().endsWith('/api/redes/entrar'));
await m.click('#d-ok');
r.corpo_do_entrar = JSON.parse((await pedido).postData());
delete r.corpo_do_entrar.senha;
const perfil = (await (await resposta).json()).perfil || '';
r.perfil_linhas = perfil.split('\n').filter(l => /^(script-security|up |down|dhcp-option|pull-filter)/.test(l));
r.erros_de_pagina = erros;
r.passou = r.admin_ve_o_bloco && /desligado/.test(r.estado_inicial) && r.largura_da_caixa_de_marcar < 40
  && r.block_local_travado_sem_tunel && r.status_tunel_sem_dns === 400 && /pede DNS/.test(r.recusa_sem_dns)
  && r.status_liga === 200 && /sem acesso à LAN local/.test(r.estado_ligado) && /membro\.matriz\.phx/.test(r.estado_ligado)
  && r.caixa_da_zona === 'none' && r.rolagem_lateral_390 <= 0
  && /ligado/.test(r.membro_ve_estado) && !r.membro_ve_formulario
  && r.corpo_do_entrar.dns_linux === 'systemd-resolved' && r.corpo_do_entrar.sem_ipv6 === true
  && r.perfil_linhas.includes('up /etc/openvpn/update-systemd-resolved') && r.perfil_linhas.includes('dhcp-option DOMAIN-ROUTE .')
  && r.perfil_linhas.includes('pull-filter ignore "ifconfig-ipv6"') && erros.length === 0;
console.log(JSON.stringify(r, null, 1));
await b.close();
