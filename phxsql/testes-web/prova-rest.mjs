#!/usr/bin/env node
/* A PROVA da secao do webservice REST na tela de configuracao, EXERCITANDO.
 *
 *     cargo build --release -p phxsql-server --bin phxsqld
 *     node phxsql/testes-web/prova-rest.mjs --porta 7530
 *
 * # Por que esta bateria existe
 *
 * Ler o codigo nao acha o que o CSS global faz com componente novo. A casa ja
 * pagou duas vezes: `input{width:100%}` transformou uma caixa de marcar numa
 * barra de 834px, e `label{text-transform:uppercase}` fez «Blumenau» aparecer
 * como «BLUMENAU» -- que e uma mentira sobre o dado. Esta secao traz um
 * componente NOVO, o `<textarea>` da lista de tabelas, e nenhum dos dois
 * defeitos aparece lendo o fonte.
 *
 * O que ela prova, em ordem:
 *
 *   1. a secao aparece na tela de configuracao, com os sete campos;
 *   2. o `<textarea>` nao foi esticado nem gritado pelo CSS global;
 *   3. PREENCHER e SALVAR grava no config.json de verdade;
 *   4. RECARREGAR mostra o que foi gravado -- e nao o que estava valendo;
 *   5. TROCAR O IDIOMA traduz os rotulos da secao na hora;
 *   6. o token NAO aparece na tela, em nenhum idioma.
 *
 * Sobe um phxsqld so dela, na faixa 7530-7539, e o derruba pelo PID.
 */
import { chromium } from '/opt/node22/lib/node_modules/playwright/index.mjs';
import { existsSync, readFileSync, readdirSync, statSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

import { subir, USUARIO, SENHA, TOKEN } from './servidor.mjs';
import { Falha, verdade, igual, entrar } from './apoio.mjs';

const AQUI = dirname(fileURLToPath(import.meta.url));
const RAIZ = resolve(AQUI, '..');

const arg = (nome, padrao = null) => {
  const i = process.argv.indexOf(nome);
  return i > 0 && process.argv[i + 1] ? process.argv[i + 1] : padrao;
};
const PORTA = Number(arg('--porta', '7530'));

/** A mesma recusa das outras baterias: binario velho mede o passado. */
function conferirBinario(phxsqld) {
  if (!existsSync(phxsqld)) {
    throw new Error(`nao achei ${phxsqld} — rode:\n`
      + '  cargo build --release -p phxsql-server --bin phxsqld');
  }
  const doBinario = statSync(phxsqld).mtimeMs;
  const ui = join(RAIZ, 'crates/phxsql-server/ui');
  let maisNovo = null;
  const andar = d => {
    for (const nome of readdirSync(d)) {
      const p = join(d, nome);
      const st = statSync(p);
      if (st.isDirectory()) andar(p);
      else if (!maisNovo || st.mtimeMs > maisNovo.ms) maisNovo = { p, ms: st.mtimeMs };
    }
  };
  andar(ui);
  if (maisNovo && maisNovo.ms > doBinario) {
    throw new Error(`${maisNovo.p} e mais novo que o binario: a pagina esta`
      + ' EMBUTIDA, recompile antes de medir');
  }
}

const passos = [];
const ok = (nome, detalhe = '') => passos.push({ nome, ok: true, detalhe });
const mal = (nome, detalhe = '') => passos.push({ nome, ok: false, detalhe });

async function abrirConfiguracoes(page) {
  await page.evaluate(() => verConfigServidor());
  await page.waitForSelector('#painel .secao', { timeout: 15000 });
}

async function main() {
  const phxsqld = join(RAIZ, 'target/release/phxsqld');
  conferirBinario(phxsqld);
  const srv = await subir({ phxsqld, portaDados: PORTA, portaWeb: PORTA + 1 });
  const url = `http://127.0.0.1:${PORTA + 1}/`;
  const navegador = await chromium.launch();
  try {
    const page = await navegador.newPage({ viewport: { width: 1400, height: 1000 } });
    await entrar(page, url);
    await abrirConfiguracoes(page);

    // ---------------------------------------------------------------- 1
    const campos = await page.$$eval('#painel [data-campo]', els =>
      els.map(e => e.dataset.campo).filter(c => c.startsWith('rest.')));
    const esperados = ['rest.ligado', 'rest.bind', 'rest.nome', 'rest.database',
      'rest.tabelas', 'rest.swagger_ligado', 'rest.swagger_bind'];
    igual(campos.sort().join(','), esperados.sort().join(','),
      'os sete campos do REST na tela');
    ok('a secao aparece com os sete campos', campos.length + ' campos');

    // ---------------------------------------------------------------- 2
    // O componente NOVO, medido no navegador: o CSS global morde.
    const caixa = await page.$('#painel [data-campo="rest.tabelas"]');
    verdade(caixa, 'a caixa de tabelas existe');
    const medida = await caixa.evaluate(el => {
      const r = el.getBoundingClientRect();
      const cs = getComputedStyle(el);
      const rot = el.closest('label')?.querySelector('span');
      return {
        etiqueta: el.tagName,
        largura: Math.round(r.width),
        altura: Math.round(r.height),
        caixaAlta: rot ? getComputedStyle(rot).textTransform : '',
        rotulo: rot ? rot.textContent.trim() : '',
      };
    });
    igual(medida.etiqueta, 'TEXTAREA', 'a lista e uma caixa de texto de varias linhas');
    verdade(medida.altura > 50, `a caixa tem ${medida.altura}px de altura -- `
      + 'esmagada a uma linha ninguem digita cinco tabelas');
    verdade(medida.largura < 900, `a caixa tem ${medida.largura}px de largura -- `
      + 'esticada pelo input{width:100%} global ela atravessaria a tela');
    ok('o textarea nao foi esmagado nem esticado pelo CSS global',
      `${medida.largura}x${medida.altura}px`);

    // ---------------------------------------------------------------- 3
    await page.fill('#painel [data-campo="rest.bind"]', `127.0.0.1:${PORTA + 5}`);
    await page.fill('#painel [data-campo="rest.nome"]', 'Loja do Adriano');
    await page.fill('#painel [data-campo="rest.database"]', 'loja');
    await page.fill('#painel [data-campo="rest.tabelas"]', 'clientes\npedidos\n\n  itens  ');
    await page.check('#painel [data-campo="rest.ligado"]');
    await page.click('#cfSalvar');
    await page.waitForTimeout(1500);

    const arquivo = JSON.parse(readFileSync(join(srv.dir, 'config.json'), 'utf8'));
    igual(String(arquivo.rest?.ligado), 'true', 'o ligado gravou');
    igual(arquivo.rest?.nome, 'Loja do Adriano', 'o nome gravou');
    igual(arquivo.rest?.database, 'loja', 'o banco gravou');
    igual(JSON.stringify(arquivo.rest?.tabelas), '["clientes","pedidos","itens"]',
      'a lista gravou como LISTA, sem a linha em branco e sem o espaco');
    ok('salvar grava no config.json', JSON.stringify(arquivo.rest));

    // ---------------------------------------------------------------- 4
    await abrirConfiguracoes(page);
    const relido = await page.$eval('#painel [data-campo="rest.tabelas"]', el => el.value);
    igual(relido, 'clientes\npedidos\nitens', 'a tela mostra o que foi GRAVADO');
    const nomeRelido = await page.$eval('#painel [data-campo="rest.nome"]', el => el.value);
    igual(nomeRelido, 'Loja do Adriano', 'o nome voltou como foi digitado');
    // «Loja do Adriano» e DADO: a tela nao pode grita-lo em caixa alta.
    const gritou = await page.$eval('#painel [data-campo="rest.nome"]',
      el => getComputedStyle(el).textTransform);
    igual(gritou, 'none', 'o nome digitado nao vira caixa alta -- dado nao se enfeita');
    ok('recarregar mostra o gravado, e sem gritar o dado', relido.replace(/\n/g, '|'));

    // ---------------------------------------------------------------- 5
    const rotuloPt = await page.$eval('#painel [data-campo="rest.tabelas"]',
      el => el.closest('label').querySelector('span').textContent.trim());
    await page.evaluate(() => escolherIdioma('Ingles'));
    await page.waitForTimeout(400);
    await abrirConfiguracoes(page);
    const rotuloEn = await page.$eval('#painel [data-campo="rest.tabelas"]',
      el => el.closest('label').querySelector('span').textContent.trim());
    verdade(rotuloEn !== rotuloPt,
      `o rotulo nao mudou de idioma: «${rotuloPt}» continuou «${rotuloEn}»`);
    verdade(/table/i.test(rotuloEn), `o rotulo em ingles saiu «${rotuloEn}»`);
    const tituloEn = await page.$$eval('#painel .secao', els =>
      els.map(e => e.textContent.trim()).filter(t => /REST/.test(t))[0] || '');
    verdade(/REST/.test(tituloEn), `o titulo da secao sumiu: «${tituloEn}»`);
    ok('trocar o idioma traduz a secao na hora', `${rotuloPt} -> ${rotuloEn}`);

    // ---------------------------------------------------------------- 6
    // E o dado continua dado: o nome do servico nao se traduz.
    const nomeEn = await page.$eval('#painel [data-campo="rest.nome"]', el => el.value);
    igual(nomeEn, 'Loja do Adriano', 'o nome do servico NAO se traduz -- e dado');
    // O token nao esta em campo nenhum da tela -- nem para ler, nem para
    // editar. Conferido pelos VALORES dos controles, e nao pelo texto da
    // pagina: o diretorio temporario da bateria carrega o nome dela, e um
    // `innerText.includes(token)` acusaria o caminho do banco.
    const valores = await page.$$eval('#painel input, #painel textarea',
      els => els.map(e => e.value));
    verdade(!valores.includes(TOKEN_NA_TELA),
      'o token do servidor esta num campo da tela');
    const secao = await page.evaluate(() => {
      const c = [...document.querySelectorAll('#painel .nota')]
        .map(e => e.textContent).filter(t => /token/i.test(t));
      return c.join(' ');
    });
    verdade(/own token|token/i.test(secao), 'a secao nao diz nada sobre o token');
    verdade(!secao.includes(TOKEN_NA_TELA), 'o rodape mostrou o token');
    ok('o token nunca aparece -- a tela so diz que nao ha um proprio',
      secao.slice(0, 90));

    await page.evaluate(() => escolherIdioma('Portugues'));
  } finally {
    await navegador.close();
    await srv.derrubar();
  }
}

// O token e o do servidor da bateria; a pagina nunca deveria mostra-lo.
globalThis.TOKEN_NA_TELA = TOKEN;

let codigo = 0;
try {
  await main();
} catch (e) {
  mal('a bateria parou', e instanceof Falha ? e.message : String(e));
  codigo = 1;
}
console.log('\n== a secao do REST na tela de configuracao ==');
for (const p of passos) {
  console.log(`  ${p.ok ? 'OK  ' : 'FALHA'}  ${p.nome}${p.detalhe ? '  -- ' + p.detalhe : ''}`);
}
console.log(`\n  ${passos.filter(p => p.ok).length}/${passos.length} passos`);
process.exit(codigo || (passos.some(p => !p.ok) ? 1 : 0));
