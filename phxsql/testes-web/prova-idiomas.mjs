#!/usr/bin/env node
/* A PROVA do multi-idioma, pelo navegador -- e nao por leitura de codigo.
 *
 *     cargo build --release -p phxsql-server --bin phxsqld
 *     node phxsql/testes-web/prova-idiomas.mjs --capturas /tmp/idiomas
 *
 * O que ela prova, em ordem:
 *
 *   1. sem escolher nada, a tela e a de sempre -- em portugues. E o teste do
 *      comportamento VELHO, e e o que mais importa: guarda nova entra pedida.
 *   2. a bandeira da tela de ENTRADA troca o texto do login sem recarregar;
 *   3. a escolha SOBREVIVE ao login -- entrar leva o idioma junto;
 *   4. a bandeira da tela de CONFIGURACAO troca o cromo (menu, barra, abas,
 *      arvore) na hora, sem recarregar, e sem levar a pessoa para outra tela;
 *   5. a escolha sobrevive a SAIR e entrar de novo;
 *   6. o alemao NAO estoura o botao da barra -- o texto que estica e o
 *      defeito que so aparece traduzindo.
 *
 * Sobe um phxsqld so dela, na faixa 6650-6699, e o derruba pelo PID.
 */
import { chromium } from '/opt/node22/lib/node_modules/playwright/index.mjs';
import { existsSync, mkdirSync, readdirSync, statSync } from 'node:fs';
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
const opc = {
  capturas: arg('--capturas'),
  porta: Number(arg('--porta', '6650')),
  ver: process.argv.includes('--ver'),
};

/** A mesma recusa da bateria: pagina embutida, binario velho mede o passado. */
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
    throw new Error(`o phxsqld e mais velho que ${maisNovo.p} — recompile antes`);
  }
}

const passos = [];
async function passo(nome, f) {
  const t0 = Date.now();
  try {
    await f();
    passos.push({ nome, ok: true, ms: Date.now() - t0 });
    console.log(`  \x1b[32mok    \x1b[0m ${nome.padEnd(46)} ${Date.now() - t0} ms`);
  } catch (e) {
    passos.push({ nome, ok: false, erro: e });
    console.log(`  \x1b[31mFALHOU\x1b[0m ${nome}\n         ${e.message}`);
  }
}

async function capturar(page, nome) {
  if (!opc.capturas) return;
  mkdirSync(opc.capturas, { recursive: true });
  await page.screenshot({ path: join(opc.capturas, `${nome}.png`), fullPage: false });
}

/** Clica a bandeira de um idioma no seletor pedido e espera o texto trocar. */
async function escolher(page, seletor, col) {
  await page.click(`${seletor} .idi[data-idi="${col}"]`);
  await page.waitForFunction(
    c => typeof est === 'object' && est.textos && est.textos['tela.entrar']
      && document.documentElement.lang !== '' && localStorage.getItem('phxsql-idioma') === c,
    col, { timeout: 10000 });
  await page.waitForTimeout(250);
}

const main = async () => {
  const phxsqld = join(RAIZ, 'target/release/phxsqld');
  conferirBinario(phxsqld);
  const srv = await subir({
    phxsqld, portaDados: opc.porta, portaWeb: opc.porta + 1,
    log: m => console.log(`\x1b[90m· ${m}\x1b[0m`),
  });
  const navegador = await chromium.launch({ headless: !opc.ver });
  const ctx = await navegador.newContext({ viewport: { width: 1440, height: 900 } });
  const page = await ctx.newPage();
  const erros = [];
  page.on('pageerror', e => erros.push(String(e)));

  try {
    // ---------------------------------------------------------- 1. o velho
    await passo('sem escolher nada, a tela e a de sempre (portugues)', async () => {
      await page.goto(srv.url, { waitUntil: 'domcontentloaded' });
      await page.waitForSelector('#btEntrar');
      igual(await page.textContent('#btEntrar'), 'Entrar', 'o botao de entrar');
      igual(await page.textContent('[data-txt="tela.servidor"]'), 'Servidor', 'o rotulo do servidor');
      await capturar(page, '01-login-portugues');
    });

    // -------------------------------------------- 2. a bandeira do login
    await passo('a bandeira da tela de entrada troca o texto na hora', async () => {
      await escolher(page, '#idiomas', 'Ingles');
      igual(await page.textContent('#btEntrar'), 'Sign in', 'o botao de entrar em ingles');
      igual(await page.textContent('[data-txt="tela.servidor"]'), 'Server', 'o rotulo do servidor');
      igual(await page.getAttribute('html', 'lang'), 'en', 'o lang do documento');
      await capturar(page, '02-login-ingles');
    });

    // ------------------------------------ 3. a escolha sobrevive ao login
    await passo('a escolha atravessa o login: o cromo entra em ingles', async () => {
      await page.fill('#u', USUARIO);
      await page.fill('#s', SENHA);
      await page.fill('#t', TOKEN);
      await page.click('#btEntrar');
      await page.waitForSelector('#app.ativo', { timeout: 20000 });
      await page.waitForSelector('#arvore .no', { timeout: 20000 });
      const menus = await page.$$eval('.menubar .titulo', ns => ns.map(n => n.textContent.trim()));
      verdade(menus.includes('File'), `o menu Arquivo devia dizer File: ${menus.join(', ')}`);
      verdade(menus.includes('Settings'), `faltou Settings: ${menus.join(', ')}`);
      const arvore = await page.textContent('#arvore');
      verdade(arvore.includes('Databases'), 'a arvore devia dizer Databases');
      igual(await page.textContent('#btSair'), 'Sign out', 'o botao de sair');
      await capturar(page, '03-app-ingles');
    });

    // ---------------------------- 4. o outro caminho: a tela de configuracao
    await passo('a tela de Configuracoes troca o idioma e NAO muda de tela', async () => {
      await page.click('.fer[title^="Config"], .fer[title^="Konfig"], #ferramentas .fer >> nth=13');
      await page.waitForSelector('#idiomasAqui .idi', { timeout: 10000 });
      igual(await page.textContent('#titulo'), 'General server settings',
        'a tela de configuracoes em ingles');
      await escolher(page, '#idiomasAqui', 'Portugues');
      await page.waitForSelector('#idiomasAqui .idi', { timeout: 10000 });
      // Continua sendo a MESMA tela -- so que em portugues. O gancho
      // `est.repintar` existe exatamente para isto: sem ele, trocar o idioma
      // aqui devolveria a pessoa ao Painel.
      igual(await page.textContent('#titulo'), 'Configurações gerais do servidor',
        'a tela tem de continuar a de configuracoes, agora em portugues');
      verdade(await page.$('#cfSalvar') !== null, 'o botao de salvar do config sumiu');
      const menus = await page.$$eval('.menubar .titulo', ns => ns.map(n => n.textContent.trim()));
      verdade(menus.includes('Arquivo'), `o menu devia voltar ao portugues: ${menus.join(', ')}`);
      await capturar(page, '04-config-portugues');
    });

    // ------------------------------------------ 5. a escolha sobrevive a sair
    await passo('a escolha sobrevive a sair e entrar de novo', async () => {
      await page.click('#idiomasAqui .idi[data-idi="Alemao"]');
      await page.waitForFunction(
        () => localStorage.getItem('phxsql-idioma') === 'Alemao', null, { timeout: 10000 });
      await page.goto(srv.url, { waitUntil: 'domcontentloaded' });
      await page.waitForSelector('#btEntrar');
      await page.waitForFunction(
        () => document.querySelector('#btEntrar').textContent.trim() === 'Anmelden',
        null, { timeout: 10000 });
      igual(await page.getAttribute('html', 'lang'), 'de', 'o lang do documento');
      await capturar(page, '05-login-alemao');
    });

    // ------------------------------------------------ 6. o texto que estica
    await passo('o alemao nao estoura o botao da barra de ferramentas', async () => {
      await entrar(page, srv.url);
      await page.waitForSelector('#ferramentas .fer');
      const estouros = await page.$$eval('#ferramentas .fer .rot', ns => ns
        .filter(n => n.scrollWidth > n.clientWidth + 1)
        .map(n => `${n.textContent.trim()} (${n.scrollWidth}>${n.clientWidth})`));
      // Reticencias no rotulo da barra e CORTE, e corte esconde o nome do
      // botao. Aqui o rotulo tem de caber inteiro.
      verdade(estouros.length === 0, `rotulo cortado em alemao: ${estouros.join(', ')}`);
      const larguras = await page.$$eval('#ferramentas .fer',
        ns => ns.map(n => Math.round(n.getBoundingClientRect().width)));
      console.log(`\x1b[90m      nota: ${larguras.length} botoes de barra, o mais largo `
        + `${Math.max(...larguras)}px\x1b[0m`);
      const rolaDeLado = await page.evaluate(() =>
        document.body.scrollWidth > document.body.clientWidth + 1);
      verdade(!rolaDeLado, 'a pagina rolou de lado com o texto alemao');
      await capturar(page, '06-app-alemao');
      await page.click('#ferramentas .fer >> nth=13');
      await page.waitForSelector('#idiomasAqui .idi', { timeout: 10000 });
      await capturar(page, '07-config-alemao');
    });

    // -------------------------------- 7. a mesma tela, os dois temas, 2 linguas
    await passo('a mesma tela em dois idiomas e nos dois temas', async () => {
      for (const [col, nome] of [['Portugues', 'pt'], ['Ingles', 'en'], ['Alemao', 'de']]) {
        await escolher(page, '#idiomasAqui', col);
        await page.waitForSelector('#idiomasAqui .idi');
        for (const tema of ['escuro', 'claro']) {
          const atual = await page.evaluate(() => document.documentElement.dataset.tema);
          if (atual !== tema) await page.click('#btTema');
          await page.waitForTimeout(150);
          await capturar(page, `08-config-${nome}-${tema}`);
        }
      }
    });

    verdade(erros.length === 0, `excecao na pagina: ${erros.join(' | ')}`);
  } finally {
    await navegador.close();
    await srv.derrubar();
    console.log('\x1b[90m· servidor derrubado pelo PID\x1b[0m');
  }

  const maus = passos.filter(p => !p.ok);
  console.log(`\n${passos.length - maus.length}/${passos.length} passos passaram`);
  if (maus.length) process.exit(1);
};

main().catch(e => { console.error(e); process.exit(1); });
