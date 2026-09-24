// A tela de gestao (pedidos 456/457/458) exercitada no navegador: ana troca
// a propria senha e entra de novo com a nova; o admin desativa e reativa a
// ana pelo botao vermelho/verde (com confirmacao DENTRO da pagina, nunca
// confirm()); o admin remove a ana da rede Matriz, tambem com confirmacao.
import { chromium } from '/opt/node22/lib/node_modules/playwright/index.mjs';

const URL = process.env.PAINEL, SAIDA = process.env.SAIDA;
const LARGURA = Number(process.env.LARGURA || 900), ALTURA = Number(process.env.ALTURA || 900);
const PREFIXO = process.env.PREFIXO || 'gestao';
const b = await chromium.launch();
const p = await b.newPage({ viewport: { width: LARGURA, height: ALTURA } });
const erros = [];
p.on('pageerror', e => erros.push(e.message));
p.on('console', m => { if (m.type() === 'error' && !/status of 40[01]/.test(m.text())) erros.push(m.text()); });
// Nenhum confirm()/prompt() do navegador pode aparecer nesta tela -- se
// aparecer, o dialog handler abaixo o aceita e ISTO conta como falha (achado
// exercitando: sem o handler o Playwright trava esperando o dialogo).
let dialogoNativo = null;
p.on('dialog', d => { dialogoNativo = d.type(); d.dismiss(); });

const r = {};
const entrar = async (u, s) => {
  await p.fill('#f-login [name=usuario]', u);
  await p.fill('#f-login [name=senha]', s);
  // Limpa a mensagem da tentativa ANTERIOR -- senao a espera abaixo acha o
  // texto velho e segue em frente antes da resposta desta tentativa chegar
  // (achado exercitando: a senha nova "nao entrava" por causa disso, nao da
  // senha).
  await p.$eval('#m-login', m => { m.textContent = ''; });
  await p.click('#f-login button[type=submit]');
  await p.waitForFunction(() => !document.querySelector('#tela-redes').hidden || document.querySelector('#m-login').textContent, null, { timeout: 30000 });
};

await p.goto(URL);
await p.waitForSelector('#tela-login:not([hidden])');

// --- 1) Trocar a propria senha (ana).
await entrar('ana', 'senha-da-ana-longa');
await p.click('#b-senha');
await p.waitForSelector('#tela-senha:not([hidden])');
await p.screenshot({ path: `${SAIDA}/${PREFIXO}-senha-antes.png`, fullPage: true });
await p.fill('#f-senha [name=senha_atual]', 'senha-da-ana-longa');
await p.fill('#f-senha [name=senha_nova]', 'senha-nova-da-ana-longa');
await p.click('#f-senha button[type=submit]');
await p.waitForFunction(() => document.querySelector('#m-senha').textContent.length > 0, null, { timeout: 15000 });
r.msg_troca = await p.textContent('#m-senha');
await p.screenshot({ path: `${SAIDA}/${PREFIXO}-senha-depois.png`, fullPage: true });
await p.click('#b-logout');
await p.waitForSelector('#tela-login:not([hidden])');
// A senha VELHA nao entra mais.
await entrar('ana', 'senha-da-ana-longa');
r.velha_senha_recusada = await p.textContent('#m-login');
// A NOVA entra.
await entrar('ana', 'senha-nova-da-ana-longa');
r.nova_senha_entrou = await p.isVisible('#tela-redes');
await p.click('#b-logout');
await p.waitForSelector('#tela-login:not([hidden])');

// --- 2) Admin desativa e reativa a ana.
await entrar('admin', 'senha-admin-longa');
await p.click('#b-admin');
await p.waitForSelector('#tela-admin:not([hidden])');
const linhaAna = () => p.locator('#t-usuarios tr', { hasText: 'ana' });
r.linha_antes = await linhaAna().textContent();
await linhaAna().locator('button.exclui', { hasText: 'Desativar' }).click();
await p.waitForSelector('#d-confirmar[open]');
r.texto_confirmacao_desativar = await p.textContent('#dc-texto');
await p.screenshot({ path: `${SAIDA}/${PREFIXO}-confirmar-desativar.png`, fullPage: true });
await p.click('#dc-ok');
await p.waitForFunction(() => document.querySelector('#t-usuarios').textContent.includes('desativado'), null, { timeout: 15000 });
r.linha_depois_desativar = await linhaAna().textContent();
await p.screenshot({ path: `${SAIDA}/${PREFIXO}-admin-desativado.png`, fullPage: true });
// ana desativada nao entra mais.
await p.click('#b-logout');
await p.waitForSelector('#tela-login:not([hidden])');
await entrar('ana', 'senha-nova-da-ana-longa');
r.desativada_nao_entra = await p.textContent('#m-login');
await entrar('admin', 'senha-admin-longa');
await p.click('#b-admin');
await p.waitForSelector('#tela-admin:not([hidden])');
await linhaAna().locator('button.inclui', { hasText: 'Reativar' }).click();
await p.waitForSelector('#d-confirmar[open]');
await p.click('#dc-ok');
await p.waitForFunction(() => document.querySelector('#t-usuarios').textContent.includes('| ativo') || /ana[\s\S]*ativo/.test(document.querySelector('#t-usuarios').textContent), null, { timeout: 15000 }).catch(() => {});
r.linha_depois_reativar = await linhaAna().textContent();

// --- 3) Remover a ana da rede Matriz (ela precisa estar ativa de novo).
await p.click('#b-voltar');
await p.waitForSelector('#tela-redes:not([hidden])');
await p.waitForFunction(() => document.querySelector('#lista-redes').textContent.includes('ana'), null, { timeout: 15000 });
await p.screenshot({ path: `${SAIDA}/${PREFIXO}-redes-antes-remover.png`, fullPage: true });
const linhaMembro = p.locator('#lista-redes .membro', { hasText: 'ana' });
await linhaMembro.locator('button.exclui', { hasText: 'Remover' }).click();
await p.waitForSelector('#d-confirmar[open]');
r.texto_confirmacao_remover = await p.textContent('#dc-texto');
await p.click('#dc-ok');
await p.waitForFunction(() => !document.querySelector('#lista-redes').textContent.includes('ana'), null, { timeout: 15000 });
r.ana_sumiu_da_rede = !(await p.textContent('#lista-redes')).includes('ana');
await p.screenshot({ path: `${SAIDA}/${PREFIXO}-redes-depois-remover.png`, fullPage: true });

// --- 4) O favicon/logo novo (pedido 6 da rodada): PNG proprio, nao mais o
// SVG antigo -- e sem embutir o de 4 MB.
r.favicon = await p.$eval('link[rel=icon]', l => l.href);
const logo = await p.request.get(`${URL}logo-128.png`.replace('//logo', '/logo'));
r.logo_status = logo.status();
r.logo_bytes = (await logo.body()).length;

r.dialogo_nativo_apareceu = dialogoNativo;
r.erros_de_console = erros;
console.log(JSON.stringify(r, null, 1));
if (erros.length || dialogoNativo) process.exitCode = 1;
await b.close();
