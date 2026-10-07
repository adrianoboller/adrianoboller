#!/usr/bin/env node
/* Prova real, no navegador, do pedido 667: a tela aberta por http:// fora do
 * loopback nao manda a senha.
 *
 *     node testes-web/prova-667-senha-pela-lan.mjs [--binario <phxsqld>] [--porta <n>]
 *
 * A tela e aberta pelo IP da MAQUINA na rede, e nao pelo 127.0.0.1: e so ai
 * que o Chromium tira o `crypto.subtle` (contexto nao seguro) e a pagina cai
 * na reserva Base64. Pelo localhost a prova nao prova nada -- la o contexto
 * e seguro e o desafio-resposta sempre roda.
 *
 * O dano se mede no FIO, e nao no recado: todo POST que a pagina faz e
 * capturado, e o vermelho e a senha (ou o `senha_b64`) aparecer em algum.
 * Com o binario de ANTES do conserto (a pagina e embutida por
 * `include_str!`), o login sai com `senha_b64` decodificavel.
 *
 * O controle e o mesmo servidor pelo 127.0.0.1: la a entrada tem de dar
 * certo, senao o verde de cima poderia ser so uma pagina quebrada.
 *
 * Sai 0 quando tudo vale, 1 quando algo falha. */
import { chromium } from '/opt/node22/lib/node_modules/playwright/index.mjs';
import { networkInterfaces } from 'node:os';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

import { subir, SENHA, USUARIO, TOKEN } from './servidor.mjs';

const AQUI = dirname(fileURLToPath(import.meta.url));
const arg = (nome, padrao) => {
  const i = process.argv.indexOf(nome);
  return i >= 0 ? process.argv[i + 1] : padrao;
};
const BINARIO = resolve(arg('--binario', resolve(AQUI, '..', 'target', 'debug', 'phxsqld')));
const PORTA = Number(arg('--porta', '6346'));

/* O primeiro IPv4 que nao e loopback. Sem ele a prova NAO RODA, e diz. */
const ipDaRede = Object.values(networkInterfaces()).flat()
  .find(i => i && i.family === 'IPv4' && !i.internal)?.address;
if (!ipDaRede) {
  console.log('NAO RODOU: esta maquina nao tem IPv4 fora do loopback');
  process.exit(1);
}

const resultados = [];
function registrar(id, ok, detalhe) {
  resultados.push({ id, ok, detalhe });
  console.log(`${ok ? 'VERDE   ' : 'VERMELHO'} ${id} -- ${detalhe}`);
}

const b64 = s => Buffer.from(s, 'utf8').toString('base64');

async function tentarEntrar(navegador, url) {
  const ctx = await navegador.newContext();
  const page = await ctx.newPage();
  const corpos = [];
  page.on('request', r => {
    if (r.method() === 'POST') corpos.push(r.postData() || '');
  });
  await page.goto(url, { waitUntil: 'domcontentloaded' });
  await page.waitForSelector('#btEntrar');
  await page.waitForFunction(() => typeof est === 'object' && est.demo === false, undefined,
    { timeout: 15000 });
  const seguro = await page.evaluate(() => window.isSecureContext);
  const modo = await page.$eval('#modo', e => e.textContent);
  await page.fill('#u', USUARIO);
  await page.fill('#s', SENHA);
  await page.fill('#t', TOKEN);
  await page.click('#btEntrar');
  await page.waitForFunction(() => {
    const r = document.querySelector('#recado');
    return (r && r.className.includes('erro')) || document.querySelector('#app.ativo');
  }, undefined, { timeout: 30000 });
  const entrou = await page.evaluate(() => !!document.querySelector('#app.ativo'));
  const recado = await page.$eval('#recado', e => e.textContent);
  await ctx.close();
  return { seguro, modo, corpos, entrou, recado };
}

const srv = await subir({ phxsqld: BINARIO, portaDados: PORTA, portaWeb: PORTA + 1, hostWeb: '0.0.0.0' });
const navegador = await chromium.launch();
try {
  const lan = await tentarEntrar(navegador, `http://${ipDaRede}:${PORTA + 1}/`);
  registrar('o_contexto_pela_lan_e_inseguro', lan.seguro === false,
    `isSecureContext=${lan.seguro} em http://${ipDaRede}`);
  const vazou = lan.corpos.filter(c => c.includes('senha_b64') || c.includes(SENHA)
    || c.includes(b64(SENHA)));
  registrar('a_senha_nao_sai_pela_lan', vazou.length === 0,
    `POSTs com a senha: ${vazou.length} de ${lan.corpos.length}`
    + (vazou.length ? ` -- ${JSON.stringify(vazou[0].slice(0, 160))}` : ''));
  registrar('a_tela_diz_o_porque', !lan.entrou && /n[aã]o sa/i.test(lan.recado)
    && /n[aã]o sai/i.test(lan.modo),
    `recado=${JSON.stringify(lan.recado)}; aviso=${JSON.stringify(lan.modo.slice(0, 120))}`);

  const local = await tentarEntrar(navegador, `http://127.0.0.1:${PORTA + 1}/`);
  registrar('controle_pelo_localhost_entra', local.seguro === true && local.entrou,
    `isSecureContext=${local.seguro}, entrou=${local.entrou}, recado=${JSON.stringify(local.recado)}`);
} finally {
  await navegador.close();
  await srv.derrubar?.();
}

const falhas = resultados.filter(r => !r.ok).length;
console.log(`${resultados.length - falhas}/${resultados.length} verdes`);
process.exit(falhas ? 1 : 0);
