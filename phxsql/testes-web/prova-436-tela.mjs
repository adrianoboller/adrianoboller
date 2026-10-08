#!/usr/bin/env node
/* Prova real, no navegador, dos tres achados de TELA do pedido 436 (revisao
 * SEC da noite de 23/09/2026): M5, B4 e B5.
 *
 *     node testes-web/prova-436-tela.mjs [--binario <phxsqld>] [--porta <n>]
 *
 * Sai 0 quando os tres estao consertados, 1 quando algum ainda vale. Com o
 * binario de ANTES do conserto (a pagina e embutida por `include_str!`), os
 * tres reprovam -- e esse e o vermelho: interface so se prova exercitando.
 *
 * M5 -- `perguntar()` nao consultava `oficial(c)`. Planta-se no `localStorage`
 *       um endereco na PROPRIA origem (que a `connect-src 'self'` deixa
 *       passar) e uma chave; aperta-se «Testar a chave». O dano medido e o
 *       pedido que sai com `x-api-key` -- e nao o recado da tela.
 *
 * B4 -- a tela dizia que a chave «some ao fechar a aba», e a janela aberta
 *       pelo `window.open` nascia com COPIA do `sessionStorage`. Desde o
 *       339(a) refeito (08/10/2026) a chave so mora em memoria: o verde e a
 *       janela NAO herdar a chave e a frase dizer que ela cai ao recarregar.
 *
 * B5 -- login recusado deixava a chave privada Ed25519 em `#k`. Mede-se o
 *       `.value` (nunca o `page.content()`, que nao traz valor de `<input>`:
 *       a armadilha ja paga no 339). */
import { chromium } from '/opt/node22/lib/node_modules/playwright/index.mjs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

import { subir } from './servidor.mjs';
import { entrar } from './apoio.mjs';
import { definirIA, abrirConfigClaude, testarChave } from './claude-apoio.mjs';

const AQUI = dirname(fileURLToPath(import.meta.url));
const arg = (nome, padrao) => {
  const i = process.argv.indexOf(nome);
  return i >= 0 ? process.argv[i + 1] : padrao;
};
const BINARIO = resolve(arg('--binario', resolve(AQUI, '..', 'target', 'debug', 'phxsqld')));
const PORTA = Number(arg('--porta', '6340'));

const CHAVE_IA = 'sk-ant-chave-de-prova-0436';
const CHAVE_ED = 'ab'.repeat(32);

const location_x = porta => `http://127.0.0.1:${porta + 1}/x`;

const resultados = [];
function registrar(id, ok, detalhe) {
  resultados.push({ id, ok, detalhe });
  console.log(`${ok ? 'VERDE   ' : 'VERMELHO'} ${id} -- ${detalhe}`);
}

const srv = await subir({ phxsqld: BINARIO, portaDados: PORTA, portaWeb: PORTA + 1 });
const url = `http://127.0.0.1:${PORTA + 1}/`;
const navegador = await chromium.launch();
try {
  // ------------------------------------------------------------------ B5
  {
    const ctx = await navegador.newContext();
    const page = await ctx.newPage();
    await page.goto(url, { waitUntil: 'domcontentloaded' });
    await page.waitForSelector('#btEntrar');
    await page.waitForFunction(() => typeof est === 'object' && est.demo === false, { timeout: 15000 });
    await page.fill('#u', 'adm');
    await page.fill('#s', 'senha-errada');
    await page.fill('#t', 'bateria');
    // O campo pode estar escondido nesta montagem; o valor e o que conta.
    await page.$eval('#k', (e, v) => { e.value = v; }, CHAVE_ED);
    await page.click('#btEntrar');
    await page.waitForFunction(() => document.querySelector('#recado')?.className.includes('erro'),
      undefined, { timeout: 30000 });
    const k = await page.$eval('#k', e => e.value);
    const s = await page.$eval('#s', e => e.value);
    const recado = await page.$eval('#recado', e => e.textContent);
    registrar('B5_login_recusado_tira_a_chave_privada', k === '' && s === 'senha-errada',
      `#k=${JSON.stringify(k.slice(0, 8))}… (${k.length} car.), #s ficou=${s === 'senha-errada'}, recado=${JSON.stringify(recado)}`);
    await ctx.close();
  }

  // ------------------------------------------------------------------ M5
  {
    const ctx = await navegador.newContext();
    const page = await ctx.newPage();
    const comChave = [];
    page.on('request', r => {
      const h = r.headers();
      // O endereco OFICIAL nao conta: a chave plantada indo para a Anthropic
      // e a chave de quem a plantou indo para onde ela vale. O dano e a chave
      // saindo para outro lugar.
      if (h['x-api-key'] && !r.url().startsWith('https://api.anthropic.com/')) comChave.push(r.url());
    });
    await entrar(page, url);
    // O plantio: no DISCO, que e onde um endereco sobrevive a aba e chega a
    // toda aba nova. Na propria origem, que a politica da pagina deixa passar.
    await page.evaluate(([chave]) => {
      localStorage.setItem('phxsql.ia', JSON.stringify({
        endpoint: location.origin + '/x', chave, ligado: true }));
    }, [CHAVE_IA]);
    await abrirConfigClaude(page);
    const veredito = await testarChave(page);
    registrar('M5_endereco_plantado_nao_leva_a_chave', comChave.length === 0,
      `pedidos com x-api-key: ${JSON.stringify(comChave)}; recado: ${JSON.stringify(veredito.texto.slice(0, 120))}`);

    // O portao do `perguntar` sozinho: o endereco ja NA ABA (o caminho que
    // nao passa pelo `migrarDoDisco`), sem a marca de que foi pedido.
    comChave.length = 0;
    await definirIA(page, { endpoint: location_x(PORTA), endpoint_confirmado: '', chave: CHAVE_IA });
    await abrirConfigClaude(page);
    const v2 = await testarChave(page);
    registrar('M5_endereco_na_aba_sem_marca_nao_leva', comChave.length === 0,
      `pedidos com x-api-key: ${JSON.stringify(comChave)}; recado: ${JSON.stringify(v2.texto.slice(0, 120))}`);

    // A metade contraria: o endereco trocado POR QUERER (com a marca) ainda
    // leva a chave -- e o caminho do servidor falso da bateria da Claude.
    comChave.length = 0;
    await definirIA(page, { endpoint: `http://127.0.0.1:${PORTA + 1}/y`, chave: CHAVE_IA });
    await abrirConfigClaude(page);
    await testarChave(page);
    registrar('M5_endereco_confirmado_continua_levando', comChave.length === 1,
      `pedidos com x-api-key: ${JSON.stringify(comChave)}`);
    await ctx.close();
  }

  // ------------------------------------------------------------------ B4
  {
    const ctx = await navegador.newContext();
    const page = await ctx.newPage();
    await entrar(page, url);
    await definirIA(page, { chave: CHAVE_IA, endpoint: '' });
    const [janela] = await Promise.all([
      ctx.waitForEvent('page'),
      page.evaluate(() => { window.open(location.href, 'phxsql-prova-436'); }),
    ]);
    await janela.waitForLoadState('domcontentloaded');
    await page.close();
    // Desde 08/10/2026 (339(a) refeito) a chave mora so em MEMORIA: a
    // janela nao herda copia nenhuma, nem no `sessionStorage` nem no modulo.
    const copia = await janela.evaluate(() => sessionStorage.getItem('phxsql.ia.chave') || '');
    const naJanela = await janela.evaluate(() => (window.PhxIA && window.PhxIA._cfg().chave) || '');
    const herdou = copia.includes(CHAVE_IA) || naJanela.includes(CHAVE_IA);
    // A frase, lida na tela que a pessoa ve: tem de dizer que a chave cai
    // ao recarregar, e nao prometer copia na janela destacada.
    const p2 = await ctx.newPage();
    await entrar(p2, url);
    await definirIA(p2, { chave: CHAVE_IA });
    await abrirConfigClaude(p2);
    const frase = await p2.$eval('#iaChave + .leg', e => e.textContent);
    const diz = /recarregar/i.test(frase) && !/janela destacada/i.test(frase);
    registrar('B4_a_frase_diz_o_que_o_navegador_faz', !herdou && diz,
      `a janela destacada herdou a chave: ${herdou}; a frase diz: ${JSON.stringify(frase.trim())}`);
    await ctx.close();
  }
} finally {
  await navegador.close();
  await srv.derrubar?.();
}

const vermelhos = resultados.filter(r => !r.ok).length;
console.log(`\n${resultados.length - vermelhos} verde(s), ${vermelhos} vermelho(s)`);
process.exit(vermelhos ? 1 : 0);
